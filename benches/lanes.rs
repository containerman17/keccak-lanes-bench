//! Hand-rolled timer: per configuration, calibrate to about BENCH_MS (default 100 ms) per run,
//! one warmup run, then the best of 3 runs. Multi-thread rows run the same loop on every
//! thread (thread t pinned to CPU t) with its own data and report aggregate throughput
//! (wall time for all threads).
//! Prints CSV rows, then Markdown tables. BENCH_THREADS picks thread counts (default "1,16"),
//! BENCH_IMPL keeps only implementations whose name contains it.

use keccak_lanes_bench::{HashFn, impls};
use std::hint::black_box;
use std::sync::Barrier;
use std::time::{Duration, Instant};

const SIZES: [usize; 5] = [64, 136, 512, 1024, 4096];
const BATCHES: [usize; 9] = [1, 2, 4, 5, 8, 10, 16, 32, 64];

/// Each message is its own allocation, like trie nodes.
fn messages(size: usize, batch: usize, seed: u64) -> Vec<Vec<u8>> {
    let mut x = seed | 1;
    (0..batch)
        .map(|_| {
            (0..size)
                .map(|_| {
                    x ^= x << 13;
                    x ^= x >> 7;
                    x ^= x << 17;
                    x as u8
                })
                .collect()
        })
        .collect()
}

fn timed(hash: HashFn, msgs: &[Vec<u8>], iters: u64) -> Duration {
    let refs: Vec<&[u8]> = msgs.iter().map(Vec::as_slice).collect();
    let mut out = vec![[0u8; 32]; refs.len()];
    let t = Instant::now();
    for _ in 0..iters {
        hash(black_box(&refs), black_box(&mut out));
    }
    t.elapsed()
}

fn iters_for(hash: HashFn, msgs: &[Vec<u8>], target: Duration) -> u64 {
    let mut n = 1;
    loop {
        let t = timed(hash, msgs, n);
        if t >= target / 10 {
            return (n as f64 * target.as_secs_f64() / t.as_secs_f64()).ceil() as u64;
        }
        n *= 2;
    }
}

/// Best-of-3 wall time for `threads` threads each doing `iters` calls.
fn best_of_3(hash: HashFn, size: usize, batch: usize, threads: usize, iters: u64) -> Duration {
    if threads == 1 {
        let msgs = messages(size, batch, 1);
        timed(hash, &msgs, iters); // warmup
        return (0..3).map(|_| timed(hash, &msgs, iters)).min().unwrap();
    }
    let barrier = Barrier::new(threads + 1);
    std::thread::scope(|s| {
        for t in 0..threads {
            let barrier = &barrier;
            s.spawn(move || {
                // shortcut: assumes CPUs 0..threads are distinct physical cores (true on c8i, see README).
                core_affinity::set_for_current(core_affinity::CoreId { id: t });
                let msgs = messages(size, batch, t as u64 + 1);
                for _ in 0..4 {
                    barrier.wait();
                    timed(hash, &msgs, iters);
                    barrier.wait();
                }
            });
        }
        let mut best = Duration::MAX;
        for rep in 0..4 {
            barrier.wait();
            let t = Instant::now();
            barrier.wait();
            if rep > 0 {
                best = best.min(t.elapsed());
            }
        }
        best
    })
}

fn main() {
    let threads: Vec<usize> = std::env::var("BENCH_THREADS")
        .unwrap_or_else(|_| "1,16".into())
        .split(',')
        .map(|s| s.trim().parse().unwrap())
        .collect();
    let target =
        Duration::from_millis(std::env::var("BENCH_MS").map_or(100, |s| s.parse().unwrap()));
    let only = std::env::var("BENCH_IMPL").unwrap_or_default();
    let impls: Vec<_> = impls()
        .into_iter()
        .filter(|i| i.name.contains(&only))
        .collect();
    // results[(threads, size)][impl][batch] = (ns per message, MB/s)
    let mut results = Vec::new();
    println!("threads,impl,size,batch,ns_per_msg,mb_per_s");
    for &th in &threads {
        for size in SIZES {
            let mut rows = Vec::new();
            for imp in &impls {
                let mut row = Vec::new();
                for batch in BATCHES {
                    let iters = iters_for(imp.hash, &messages(size, batch, 1), target);
                    let wall = best_of_3(imp.hash, size, batch, th, iters);
                    let msgs = (iters * batch as u64 * th as u64) as f64;
                    let ns = wall.as_nanos() as f64 / msgs;
                    let mbs = size as f64 * 1e3 / ns;
                    println!("{th},{},{size},{batch},{ns:.2},{mbs:.1}", imp.name);
                    row.push((ns, mbs));
                }
                rows.push(row);
            }
            results.push((th, size, rows));
        }
    }
    for (th, size, rows) in &results {
        let ns_label = if *th == 1 {
            "ns per message"
        } else {
            "aggregate ns per message"
        };
        println!("\n### {size} B, {th} thread(s), {ns_label} (MB/s at batch 64)\n");
        let head: Vec<String> = BATCHES.iter().map(|b| b.to_string()).collect();
        println!("| implementation | {} | MB/s @64 |", head.join(" | "));
        println!("|---|{}---:|", "---:|".repeat(BATCHES.len()));
        for (imp, row) in impls.iter().zip(rows) {
            let cells: Vec<String> = row.iter().map(|(ns, _)| format!("{ns:.1}")).collect();
            println!(
                "| {} | {} | {:.0} |",
                imp.name,
                cells.join(" | "),
                row.last().unwrap().1
            );
        }
    }
}
