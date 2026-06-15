#include <linux/bpf.h>
#include <linux/ptrace.h>
#include <linux/types.h>

#include <bpf/bpf_helpers.h>
#include <bpf/bpf_tracing.h>

typedef struct {
    __u64 sent_bytes;
    __u64 recv_bytes;
    __u8 comm[16];
} proc_stats_t;

struct {
    __uint(type, BPF_MAP_TYPE_PERCPU_HASH);
    __type(key, __u32);
    __type(value, proc_stats_t);
    __uint(max_entries, 1024);
} STATS SEC(".maps");

SEC("kprobe/tcp_sendmsg")
int procflow_tcp_sendmsg(struct pt_regs* ctx)
{
    __u32 tgid = (__u32)(bpf_get_current_pid_tgid() >> 32);
    __u64 size = (__u64)PT_REGS_PARM3(ctx);

    proc_stats_t* curr_stat = bpf_map_lookup_elem(&STATS, &tgid);
    if (curr_stat) {
        __sync_fetch_and_add(&curr_stat->sent_bytes, size);
        return 0;
    }

    proc_stats_t new_stat = {
        .recv_bytes = 0,
        .sent_bytes = size,
    };

    bpf_get_current_comm(&new_stat.comm, sizeof(new_stat.comm));
    bpf_map_update_elem(&STATS, &tgid, &new_stat, BPF_ANY);

    return 0;
}

SEC("kprobe/tcp_cleanup_rbuf")
int procflow_tcp_cleanup_rbuf(struct pt_regs* ctx)
{
    __u32 tgid = (__u32)(bpf_get_current_pid_tgid() >> 32);
    int size = PT_REGS_PARM2(ctx);

    if (size <= 0)
        return 0;

    proc_stats_t* curr_stat = bpf_map_lookup_elem(&STATS, &tgid);
    if (curr_stat) {
        __sync_fetch_and_add(&curr_stat->recv_bytes, (__u64)size);
        return 0;
    }

    proc_stats_t new_stat = {
        .recv_bytes = (__u64)size,
        .sent_bytes = 0,
    };

    bpf_get_current_comm(&new_stat.comm, sizeof(new_stat.comm));
    bpf_map_update_elem(&STATS, &tgid, &new_stat, BPF_ANY);

    return 0;
}

char LICENSE[] SEC("license") = "GPL";
