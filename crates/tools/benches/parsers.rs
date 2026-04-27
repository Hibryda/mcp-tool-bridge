//! Performance baselines for parsers — diff (unified), lsof (-F), git_log (STX/ETX).
//! Run with: cargo bench --bench parsers
//!
//! Establishes a baseline so later changes catch >10% regressions in CI nightly.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use mcp_tool_bridge::{diff, lsof};

// ── diff parser ─────────────────────────────────────────────────────

fn make_diff(n_files: usize, hunks_per_file: usize, lines_per_hunk: usize) -> String {
    let mut s = String::new();
    for f in 0..n_files {
        s.push_str(&format!(
            "diff --git a/f{f}.rs b/f{f}.rs\n--- a/f{f}.rs\n+++ b/f{f}.rs\n"
        ));
        for h in 0..hunks_per_file {
            let start = h * 30 + 1;
            let n = lines_per_hunk;
            s.push_str(&format!("@@ -{start},{n} +{start},{n} @@\n"));
            for i in 0..lines_per_hunk {
                if i % 3 == 0 {
                    s.push_str(&format!("-old line {f}.{h}.{i}\n"));
                    s.push_str(&format!("+new line {f}.{h}.{i}\n"));
                } else {
                    s.push_str(&format!(" context line {f}.{h}.{i}\n"));
                }
            }
        }
    }
    s
}

fn bench_diff(c: &mut Criterion) {
    let mut group = c.benchmark_group("diff_parser");
    for &(files, hunks, lines) in &[(1, 1, 10), (10, 3, 10), (50, 5, 20)] {
        let input = make_diff(files, hunks, lines);
        group.throughput(Throughput::Bytes(input.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("parse", format!("{files}f_{hunks}h_{lines}l")),
            &input,
            |b, input| b.iter(|| diff::parse_unified_diff(black_box(input))),
        );
    }
    group.finish();
}

// ── lsof parser ─────────────────────────────────────────────────────

fn make_lsof_output(n_processes: usize, fds_per_process: usize) -> String {
    let mut s = String::new();
    for p in 0..n_processes {
        s.push_str(&format!("p{}\ncproc-{}\n", p + 1000, p));
        for f in 0..fds_per_process {
            s.push_str(&format!("f{f}\ntIPv4\nPTCP\nn127.0.0.1:{}\n", 8000 + f));
        }
    }
    s
}

fn bench_lsof(c: &mut Criterion) {
    let mut group = c.benchmark_group("lsof_parser");
    for &(p, f) in &[(10, 5), (100, 10), (500, 20)] {
        let input = make_lsof_output(p, f);
        group.throughput(Throughput::Bytes(input.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("parse", format!("{p}p_{f}fd")),
            &input,
            |b, input| b.iter(|| lsof::parse_lsof_output(black_box(input))),
        );
    }
    group.finish();
}

criterion_group!(benches, bench_diff, bench_lsof);
criterion_main!(benches);
