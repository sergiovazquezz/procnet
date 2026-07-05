#include "vmlinux.h"

#include <bpf/bpf_helpers.h>
#include <bpf/bpf_tracing.h>

enum Protocol { TCP = 1, UDP };

struct ProcStartEvent {
    u32 tgid;
    u8 comm[16];
};

struct StatsBytes {
    u64 sent;
    u64 recv;
};

struct ProcStats {
    struct StatsBytes tcp;
    struct StatsBytes udp;
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

static __always_inline int account_bytes(u64 sent, u64 recv,
                                         enum Protocol protocol)
{
    u32 tgid = (u32)(bpf_get_current_pid_tgid() >> 32);

    struct ProcStats* curr_stat = bpf_map_lookup_elem(&STATS, &tgid);
    if (curr_stat) {
        if (protocol == TCP) {
            curr_stat->tcp.sent += sent;
            curr_stat->tcp.recv += recv;
        } else if (protocol == UDP) {
            curr_stat->udp.sent += sent;
            curr_stat->udp.recv += recv;
        }

        return 0;
    }

    struct ProcStats new_stat = { .tcp = { .sent = 0, .recv = 0 },
                                  .udp = { .sent = 0, .recv = 0 } };
    if (protocol == TCP) {
        new_stat.tcp.sent = sent;
        new_stat.tcp.recv = recv;
    } else if (protocol == UDP) {
        new_stat.udp.sent = sent;
        new_stat.udp.recv = recv;
    }

    if (bpf_map_update_elem(&STATS, &tgid, &new_stat, BPF_NOEXIST) == 0) {
        u8 comm[16] = { 0 };
        bpf_get_current_comm(comm, sizeof(comm));

        emit_start_event(tgid, comm);

        return 0;
    }

    curr_stat = bpf_map_lookup_elem(&STATS, &tgid);
    if (curr_stat) {
        if (protocol == TCP) {
            curr_stat->tcp.sent += sent;
            curr_stat->tcp.recv += recv;
        } else if (protocol == UDP) {
            curr_stat->udp.sent += sent;
            curr_stat->udp.recv += recv;
        }
    }

    return 0;
}

// TCP
SEC("kretprobe/tcp_sendmsg")
int BPF_KRETPROBE(procnet_tcp_sendmsg, int ret)
{
    if (ret <= 0)
        return 0;

    return account_bytes((u64)ret, 0, TCP);
}

SEC("kprobe/tcp_cleanup_rbuf")
int procnet_tcp_cleanup_rbuf(struct pt_regs* ctx)
{
    int size = PT_REGS_PARM2(ctx);

    if (size <= 0)
        return 0;

    return account_bytes(0, (u64)size, TCP);
}

// UDP
SEC("kretprobe/udp_sendmsg")
int BPF_KRETPROBE(procnet_udp_sendmsg, int ret)
{
    if (ret <= 0)
        return 0;

    return account_bytes((u64)ret, 0, UDP);
}

SEC("kretprobe/udp_recvmsg")
int BPF_KRETPROBE(procnet_udp_recvmsg, int ret)
{
    if (ret <= 0)
        return 0;

    return account_bytes(0, (u64)ret, UDP);
}

// UDPv6
SEC("kretprobe/udpv6_sendmsg")
int BPF_KRETPROBE(procnet_udpv6_sendmsg, int ret)
{
    if (ret <= 0)
        return 0;

    return account_bytes((u64)ret, 0, UDP);
}

SEC("kretprobe/udpv6_recvmsg")
int BPF_KRETPROBE(procnet_udpv6_recvmsg, int ret)
{
    if (ret <= 0)
        return 0;

    return account_bytes(0, (u64)ret, UDP);
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
