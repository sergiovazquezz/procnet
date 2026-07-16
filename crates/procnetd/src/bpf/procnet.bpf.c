#ifdef __TARGET_ARCH_arm64
#include "vmlinux_arm64.h"
#else
#include "vmlinux_x86_64.h"
#endif

#include <bpf/bpf_core_read.h>
#include <bpf/bpf_endian.h>
#include <bpf/bpf_helpers.h>
#include <bpf/bpf_tracing.h>

#ifndef AF_INET
#define AF_INET 2
#endif

#ifndef AF_INET6
#define AF_INET6 10
#endif

enum Protocol { TCP = 1, UDP };

struct ProcStartEvent {
    u32 tgid;
    u8 comm[16];
};

struct StatsBytes {
    u64 sent;
    u64 recv;
};

struct StatsAddr {
    u16 dst_port;
    u16 src_port;
    // Big Endian.
    u32 dst_ipv4;
    // Big Endian.
    u8 dst_ipv6[16];
};

struct ProtocolStats {
    struct StatsBytes bytes;
    struct StatsAddr addr;
};

struct ProcStats {
    struct ProtocolStats tcp;
    struct ProtocolStats udp;
};

struct {
    __uint(type, BPF_MAP_TYPE_PERCPU_HASH);
    __type(key, u32);
    __type(value, struct ProcStats);
    __uint(max_entries, 512);
} STATS SEC(".maps");

struct {
    __uint(type, BPF_MAP_TYPE_RINGBUF);
    __uint(max_entries, 4096);
} EVENTS SEC(".maps");

struct {
    __uint(type, BPF_MAP_TYPE_LRU_HASH);
    __type(key, u64);
    __type(value, struct StatsAddr);
    __uint(max_entries, 512);
} TCP_SEND_ADDRS SEC(".maps");

struct {
    __uint(type, BPF_MAP_TYPE_LRU_HASH);
    __type(key, u64);
    __type(value, struct StatsAddr);
    __uint(max_entries, 512);
} UDP_SEND_ADDRS SEC(".maps");

struct {
    __uint(type, BPF_MAP_TYPE_LRU_HASH);
    __type(key, u64);
    __type(value, struct StatsAddr);
    __uint(max_entries, 512);
} UDP_RECV_ADDRS SEC(".maps");

static __always_inline int emit_start_event(u32 tgid, u8 comm[16])
{
    struct ProcStartEvent* event =
        bpf_ringbuf_reserve(&EVENTS, sizeof(*event), 0);

    if (!event)
        return 0;

    event->tgid = tgid;
    __builtin_memcpy(event->comm, comm, sizeof(event->comm));

    bpf_ringbuf_submit(event, 0);
    return 0;
}

static __always_inline struct ProcStats create_empty_stats()
{
    return (struct ProcStats){ 0 };
}

static __always_inline void capture_addr(const struct sock* sk,
                                         struct StatsAddr* addr)
{
    *addr = (struct StatsAddr){ 0 };

    const u16 family = BPF_CORE_READ(sk, __sk_common.skc_family);

    addr->src_port = BPF_CORE_READ(sk, __sk_common.skc_num);
    addr->dst_port = bpf_ntohs(BPF_CORE_READ(sk, __sk_common.skc_dport));

    if (family == AF_INET) {
        addr->dst_ipv4 = BPF_CORE_READ(sk, __sk_common.skc_daddr);
    } else if (family == AF_INET6) {
        struct in6_addr dst6 = BPF_CORE_READ(sk, __sk_common.skc_v6_daddr);

        __builtin_memcpy(addr->dst_ipv6, dst6.in6_u.u6_addr8,
                         sizeof(addr->dst_ipv6));
    }
}

static __always_inline void update_stats(struct ProcStats* stats,
                                         enum Protocol protocol, u64 sent,
                                         u64 recv, const struct StatsAddr* addr)
{
    if (!stats)
        return;

    if (protocol == TCP) {
        stats->tcp.bytes.sent += sent;
        stats->tcp.bytes.recv += recv;

        stats->tcp.addr = *addr;
    } else if (protocol == UDP) {
        stats->udp.bytes.sent += sent;
        stats->udp.bytes.recv += recv;

        stats->udp.addr = *addr;
    }
}

static __always_inline int account_bytes(const struct StatsAddr* addr, u64 sent,
                                         u64 recv, enum Protocol protocol)
{
    u32 tgid = (u32)(bpf_get_current_pid_tgid() >> 32);

    struct ProcStats* curr_stat = bpf_map_lookup_elem(&STATS, &tgid);
    if (curr_stat) {
        update_stats(curr_stat, protocol, sent, recv, addr);

        return 0;
    }

    struct ProcStats new_stat = create_empty_stats();

    update_stats(&new_stat, protocol, sent, recv, addr);

    if (bpf_map_update_elem(&STATS, &tgid, &new_stat, BPF_NOEXIST) == 0) {
        u8 comm[16] = { 0 };
        bpf_get_current_comm(comm, sizeof(comm));

        emit_start_event(tgid, comm);

        return 0;
    }

    curr_stat = bpf_map_lookup_elem(&STATS, &tgid);
    update_stats(curr_stat, protocol, sent, recv, addr);

    return 0;
}

SEC("kprobe/tcp_sendmsg")
int BPF_KPROBE(procnet_tcp_sendmsg_entry, struct sock* sk)
{
    u64 pid_tgid = bpf_get_current_pid_tgid();
    struct StatsAddr addr;

    capture_addr(sk, &addr);
    bpf_map_update_elem(&TCP_SEND_ADDRS, &pid_tgid, &addr, BPF_ANY);
    return 0;
}

SEC("kretprobe/tcp_sendmsg")
int BPF_KRETPROBE(procnet_tcp_sendmsg_exit, int ret)
{
    u64 pid_tgid = bpf_get_current_pid_tgid();
    struct StatsAddr* saved = bpf_map_lookup_elem(&TCP_SEND_ADDRS, &pid_tgid);

    if (!saved)
        return 0;

    struct StatsAddr addr = *saved;
    bpf_map_delete_elem(&TCP_SEND_ADDRS, &pid_tgid);

    if (ret > 0)
        account_bytes(&addr, (u64)ret, 0, TCP);

    return 0;
}

SEC("kprobe/tcp_cleanup_rbuf")
int BPF_KPROBE(procnet_tcp_cleanup_rbuf, struct sock* sk, int copied)
{
    if (copied <= 0)
        return 0;

    struct StatsAddr addr = { 0 };

    capture_addr(sk, &addr);
    return account_bytes(&addr, 0, (u64)copied, TCP);
}

// UDP
SEC("kprobe/udp_sendmsg")
int BPF_KPROBE(procnet_udp_sendmsg_entry, struct sock* sk)
{
    u64 pid_tgid = bpf_get_current_pid_tgid();
    struct StatsAddr addr;

    capture_addr(sk, &addr);
    bpf_map_update_elem(&UDP_SEND_ADDRS, &pid_tgid, &addr, BPF_ANY);
    return 0;
}

SEC("kretprobe/udp_sendmsg")
int BPF_KRETPROBE(procnet_udp_sendmsg_exit, int ret)
{
    u64 pid_tgid = bpf_get_current_pid_tgid();
    struct StatsAddr* saved = bpf_map_lookup_elem(&UDP_SEND_ADDRS, &pid_tgid);

    if (!saved)
        return 0;

    struct StatsAddr addr = *saved;
    bpf_map_delete_elem(&UDP_SEND_ADDRS, &pid_tgid);

    if (ret > 0)
        account_bytes(&addr, (u64)ret, 0, UDP);

    return 0;
}

SEC("kprobe/udp_recvmsg")
int BPF_KPROBE(procnet_udp_recvmsg_entry, struct sock* sk)
{
    u64 pid_tgid = bpf_get_current_pid_tgid();
    struct StatsAddr addr;

    capture_addr(sk, &addr);
    bpf_map_update_elem(&UDP_RECV_ADDRS, &pid_tgid, &addr, BPF_ANY);
    return 0;
}

SEC("kretprobe/udp_recvmsg")
int BPF_KRETPROBE(procnet_udp_recvmsg_exit, int ret)
{
    u64 pid_tgid = bpf_get_current_pid_tgid();
    struct StatsAddr* saved = bpf_map_lookup_elem(&UDP_RECV_ADDRS, &pid_tgid);

    if (!saved)
        return 0;

    struct StatsAddr addr = *saved;
    bpf_map_delete_elem(&UDP_RECV_ADDRS, &pid_tgid);

    if (ret > 0)
        account_bytes(&addr, 0, (u64)ret, UDP);

    return 0;
}

SEC("raw_tp/sched_process_exit")
int procnet_sched_process_exit(void* ctx)
{
    u64 pid_tgid = bpf_get_current_pid_tgid();
    u32 pid = (u32)pid_tgid;
    u32 tgid = (u32)(pid_tgid >> 32);

    // NOTE: sched_process_exit fires for threads too. Only delete when the
    // thread-group leader exits.
    if (pid != tgid)
        return 0;

    bpf_map_delete_elem(&STATS, &tgid);

    return 0;
}

char LICENSE[] SEC("license") = "GPL";
