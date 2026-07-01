#include "vmlinux.h"

#include <bpf/bpf_helpers.h>
#include <bpf/bpf_tracing.h>

struct ProcStartEvent {
    __u32 tgid;
    __u8 comm[16];
};

struct ProcStats {
    __u64 sent_bytes;
    __u64 recv_bytes;
};

struct {
    __uint(type, BPF_MAP_TYPE_PERCPU_HASH);
    __type(key, __u32);
    __type(value, struct ProcStats);
    __uint(max_entries, 512);
} STATS SEC(".maps");

struct {
    __uint(type, BPF_MAP_TYPE_RINGBUF);
    __uint(max_entries, 4096);
} EVENTS SEC(".maps");

static __always_inline int emit_start_event(__u32 tgid, __u8 comm[16])
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

static __always_inline int account_bytes(__u64 sent, __u64 recv)
{
    __u32 tgid = (__u32)(bpf_get_current_pid_tgid() >> 32);

    struct ProcStats* curr_stat = bpf_map_lookup_elem(&STATS, &tgid);
    if (curr_stat) {
        if (sent)
            __sync_fetch_and_add(&curr_stat->sent_bytes, sent);
        else if (recv)
            __sync_fetch_and_add(&curr_stat->recv_bytes, recv);

        return 0;
    }

    struct ProcStats new_stat = {
        .sent_bytes = sent,
        .recv_bytes = recv,
    };

    if (bpf_map_update_elem(&STATS, &tgid, &new_stat, BPF_NOEXIST) == 0) {
        __u8 comm[16] = { 0 };
        bpf_get_current_comm(comm, sizeof(comm));

        emit_start_event(tgid, comm);

        return 0;
    }

    curr_stat = bpf_map_lookup_elem(&STATS, &tgid);
    if (curr_stat) {
        if (sent)
            __sync_fetch_and_add(&curr_stat->sent_bytes, sent);
        else if (recv)
            __sync_fetch_and_add(&curr_stat->recv_bytes, recv);
    }

    return 0;
}

SEC("kprobe/tcp_sendmsg")
int procnet_tcp_sendmsg(struct pt_regs* ctx)
{
    __u64 size = (__u64)PT_REGS_PARM3(ctx);

    return account_bytes(size, 0);
}

SEC("kprobe/tcp_cleanup_rbuf")
int procnet_tcp_cleanup_rbuf(struct pt_regs* ctx)
{
    int size = PT_REGS_PARM2(ctx);

    if (size <= 0)
        return 0;

    return account_bytes(0, (__u64)size);
}

SEC("raw_tp/sched_process_exit")
int procnet_sched_process_exit(void* ctx)
{
    __u64 pid_tgid = bpf_get_current_pid_tgid();
    __u32 pid = (__u32)pid_tgid;
    __u32 tgid = (__u32)(pid_tgid >> 32);

    // NOTE: sched_process_exit fires for threads too. Only delete when the
    // thread-group leader exits.
    if (pid != tgid)
        return 0;

    bpf_map_delete_elem(&STATS, &tgid);

    return 0;
}

char LICENSE[] SEC("license") = "GPL";
