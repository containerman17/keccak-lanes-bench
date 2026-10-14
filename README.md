# keccak-lanes-bench

Firewood hashes trie nodes one at a time: after a proposal it walks the dirty part of the trie depth first and calls Keccak-256 once per node, children before parents. But nodes on the same level of the trie do not depend on each other, so their hashes can be computed together. One Keccak-f[1600] state is 25 lanes of 64 bits, so an AVX2 register (4 x 64 bits) can hold the same lane of 4 different messages and an AVX-512 register (8 x 64 bits) the same lane of 8. One pass of the permutation then hashes 4 or 8 nodes for little more than the cost of one. This repository measures how much that is worth on x86 against the hashing Firewood uses today, so the Firewood developers can decide whether batching node hashes is worth the change.

## What stock Firewood does

- Crate: `sha3 = "0.12.0"` (RustCrypto, `storage/Cargo.toml` and `firewood/Cargo.toml`), which resolves `keccak 0.2.0` for the permutation. On x86 that is the portable Rust permutation: no assembly, no SIMD.
- Call pattern (ethhash build, the one avalanchego links through `firewood-go-ethhash`): `Preimage::to_hash` in `storage/src/hashers/ethhash.rs` RLP-encodes one node and calls `Keccak256::digest(rlp)` if the encoding is 32 bytes or longer. It is called from `NodeStore::hash_helper_inner` in `storage/src/nodestore/hash.rs`, a depth-first recursion that hashes every unhashed child, then the node itself. One message per call, one node at a time.
- Threads: `ParallelMerkle` (`firewood/src/merkle/parallel.rs`) already runs up to 16 workers, one per first-nibble subtrie, and each worker hashes its subtrie with the same one-node-at-a-time recursion. Lanes would speed up each worker; they do not replace the workers.
- Checked at firewood commit `4966572a5` (v0.8.0).

## Implementations

All compute Keccak-256 with Ethereum (legacy, `0x01`) padding and share one signature, `fn(&[&[u8]], &mut [[u8; 32]])`. The single-message crates loop over the batch. The multi-lane ones run groups of W messages through one W-lane permutation; a group with fewer than W messages still pays for all W lanes (idle lanes hash zeros and are thrown away), so batch sizes that are not a multiple of W show the real cost.

| row | crate and version | what it is |
|---|---|---|
| sha3 0.12 (stock Firewood) | `sha3` 0.12.0 (`keccak` 0.2.2 here, 0.2.0 in Firewood's lockfile) | `Keccak256::digest`, exactly the Firewood call |
| tiny-keccak 2.0 | `tiny-keccak` 2.0.2 | `Keccak::v256`, one message per call |
| keccak-asm 0.1 | `keccak-asm` 0.1.9 | `Keccak256::digest`, CRYPTOGAMS x86-64 assembly (needs perl and a C toolchain to build) |
| keccak 0.1 simd x2 / x4 / x8 | `keccak` 0.1.6, feature `simd` (nightly `portable_simd`) | RustCrypto `f1600x2`, `f1600x4`, `f1600x8` permutations; the crate has no sponge, so `src/rustcrypto.rs` absorbs blocks with scalar code around them |
| own AVX2 x4 | this repo, `src/lanes.rs` | 4-way Keccak-f[1600] on stable `std::arch::x86_64` AVX2 intrinsics, blocks loaded with `vpgatherqq` |
| own AVX-512 x8 | this repo, `src/lanes.rs` | 8-way Keccak-f[1600] on stable AVX-512F intrinsics (stable since Rust 1.89): `vpternlogq` for the 5-way XOR and chi, `vprolq` for rotations |
| own auto | this repo | groups of 8 on the AVX-512 kernel (4 on AVX2 if there is no AVX-512), but a group of exactly one message goes to `sha3` |

The own kernels are selected at runtime with `is_x86_feature_detected!`, so they work in a normal generic x86-64 build. The 8-way kernel follows the lane layout of an earlier Go (avo) version of the same kernels: one vector per state lane, element k belongs to message k.

Correctness: `cargo test` checks three published Keccak-256 vectors (empty string, `abc`, the quick brown fox) on every implementation at batch sizes 1, 3 and 9, then a randomized test compares every implementation with `tiny-keccak`: every length from 0 to 300 bytes (all block boundaries around 136 and 272), then random lengths from 0 to 5,120 bytes, random batch sizes 1 to 20, messages at random unaligned offsets. On the benchmark box it ran 60 seconds each for the stable build, the nightly build with `rustcrypto-simd`, and the nightly `-C target-cpu=native` build (222,349 / 136,837 / 148,709 random cases): all byte-exact, nothing excluded.

## Setup

- AWS c8i.8xlarge, us-west-1b, Ubuntu 24.04: Intel Xeon 6975P-C (Granite Rapids), 16 cores / 32 threads, AVX-512 F/CD/BW/DQ/VL/IFMA/VBMI.
- Rust nightly 1.101.0-nightly (e9bbadf68 2026-10-12) for the tables (needed for the `keccak` 0.1 rows); stable 1.99.0 builds and tests everything else. Release profile, default `cargo bench` settings.
- Main tables: generic x86-64 target (no `target-cpu`), which is how Firewood is shipped. One extra table shows a `-C target-cpu=native` build, because it changes some rows a lot.
- Hand-rolled timer (`benches/lanes.rs`), not criterion: for every implementation, message size and batch size it calibrates to about 100 ms per run, does one warmup run and keeps the best of 3 runs. Every message is its own heap allocation, like trie nodes. ns per message = run time / messages hashed; MB/s = message bytes / time (10^6 bytes).
- 16 threads: every thread runs the same loop on its own data, thread t pinned to CPU t (CPUs 0 to 15 are 16 different physical cores on this box, no SMT sharing). The table shows aggregate throughput over the wall time of all 16 threads.

## Results

Column headers are the batch size (messages per call). Lower ns is better. "vs stock" is the speedup over `sha3` 0.12 at batch 64. 136 B is exactly one rate block, so the padding needs a second block: 136 B costs about twice 64 B.

### 64 B, 1 thread, ns per message

| implementation | 1 | 2 | 4 | 5 | 8 | 10 | 16 | 32 | 64 | MB/s @64 | vs stock @64 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| sha3 0.12 (stock Firewood) | 300 | 302 | 300 | 301 | 299 | 299 | 299 | 298 | 298 | 215 | 1.0x |
| tiny-keccak 2.0 | 305 | 305 | 303 | 305 | 305 | 305 | 305 | 303 | 304 | 211 | 1.0x |
| keccak-asm 0.1 | 252 | 253 | 252 | 255 | 253 | 253 | 252 | 252 | 252 | 254 | 1.2x |
| keccak 0.1 simd x2 | 500 | 250 | 249 | 298 | 249 | 249 | 249 | 249 | 249 | 257 | 1.2x |
| keccak 0.1 simd x4 | 865 | 433 | 217 | 346 | 217 | 260 | 217 | 217 | 217 | 295 | 1.4x |
| keccak 0.1 simd x8 | 1,789 | 894 | 449 | 360 | 227 | 360 | 226 | 226 | 226 | 283 | 1.3x |
| own AVX2 x4 | 446 | 224 | 113 | 179 | 113 | 135 | 113 | 113 | 113 | 568 | 2.6x |
| own AVX-512 x8 | 363 | 182 | 92.4 | 74.3 | 47.4 | 74.0 | 47.1 | 47.1 | 47.2 | 1,355 | 6.3x |
| own auto (x8; sha3 for a lone message) | 301 | 183 | 92.7 | 74.6 | 47.6 | 74.5 | 47.5 | 47.5 | 47.5 | 1,349 | 6.3x |

### 136 B, 1 thread, ns per message

| implementation | 1 | 2 | 4 | 5 | 8 | 10 | 16 | 32 | 64 | MB/s @64 | vs stock @64 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| sha3 0.12 (stock Firewood) | 570 | 572 | 570 | 574 | 572 | 571 | 571 | 570 | 568 | 240 | 1.0x |
| tiny-keccak 2.0 | 584 | 582 | 584 | 583 | 581 | 582 | 582 | 582 | 582 | 234 | 1.0x |
| keccak-asm 0.1 | 504 | 500 | 501 | 501 | 500 | 500 | 499 | 498 | 498 | 273 | 1.1x |
| keccak 0.1 simd x2 | 976 | 485 | 485 | 582 | 485 | 485 | 485 | 485 | 485 | 280 | 1.2x |
| keccak 0.1 simd x4 | 1,707 | 854 | 428 | 684 | 428 | 512 | 427 | 427 | 427 | 318 | 1.3x |
| keccak 0.1 simd x8 | 3,546 | 1,775 | 890 | 713 | 447 | 713 | 447 | 448 | 447 | 304 | 1.3x |
| own AVX2 x4 | 879 | 440 | 221 | 352 | 221 | 264 | 221 | 221 | 221 | 616 | 2.6x |
| own AVX-512 x8 | 706 | 354 | 177 | 142 | 89.2 | 142 | 89.0 | 89.2 | 89.1 | 1,526 | 6.4x |
| own auto (x8; sha3 for a lone message) | 571 | 354 | 178 | 142 | 89.5 | 142 | 89.3 | 89.4 | 89.4 | 1,522 | 6.4x |

### 512 B, 1 thread, ns per message

| implementation | 1 | 2 | 4 | 5 | 8 | 10 | 16 | 32 | 64 | MB/s @64 | vs stock @64 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| sha3 0.12 (stock Firewood) | 1,129 | 1,127 | 1,126 | 1,128 | 1,127 | 1,127 | 1,126 | 1,128 | 1,127 | 454 | 1.0x |
| tiny-keccak 2.0 | 1,135 | 1,131 | 1,138 | 1,134 | 1,135 | 1,137 | 1,135 | 1,133 | 1,134 | 452 | 1.0x |
| keccak-asm 0.1 | 991 | 990 | 991 | 991 | 990 | 989 | 989 | 989 | 990 | 517 | 1.1x |
| keccak 0.1 simd x2 | 1,936 | 966 | 967 | 1,159 | 967 | 967 | 967 | 967 | 968 | 529 | 1.2x |
| keccak 0.1 simd x4 | 3,408 | 1,703 | 852 | 1,363 | 852 | 1,022 | 852 | 852 | 853 | 600 | 1.3x |
| keccak 0.1 simd x8 | 7,074 | 3,538 | 1,773 | 1,420 | 889 | 1,420 | 890 | 890 | 891 | 574 | 1.3x |
| own AVX2 x4 | 1,740 | 870 | 436 | 696 | 436 | 523 | 436 | 436 | 437 | 1,171 | 2.6x |
| own AVX-512 x8 | 1,395 | 699 | 350 | 281 | 176 | 280 | 176 | 176 | 178 | 2,883 | 6.3x |
| own auto (x8; sha3 for a lone message) | 1,129 | 699 | 351 | 281 | 176 | 281 | 176 | 176 | 178 | 2,884 | 6.3x |

### 1 KiB, 1 thread, ns per message

| implementation | 1 | 2 | 4 | 5 | 8 | 10 | 16 | 32 | 64 | MB/s @64 | vs stock @64 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| sha3 0.12 (stock Firewood) | 2,236 | 2,244 | 2,249 | 2,242 | 2,244 | 2,244 | 2,244 | 2,246 | 2,243 | 457 | 1.0x |
| tiny-keccak 2.0 | 2,233 | 2,238 | 2,251 | 2,229 | 2,234 | 2,238 | 2,240 | 2,233 | 2,240 | 457 | 1.0x |
| keccak-asm 0.1 | 1,973 | 1,978 | 1,975 | 1,973 | 1,971 | 1,972 | 1,970 | 1,972 | 1,970 | 520 | 1.1x |
| keccak 0.1 simd x2 | 3,860 | 1,927 | 1,929 | 2,315 | 1,928 | 1,928 | 1,929 | 1,929 | 1,932 | 530 | 1.2x |
| keccak 0.1 simd x4 | 6,794 | 3,396 | 1,698 | 2,719 | 1,698 | 2,039 | 1,698 | 1,700 | 1,702 | 602 | 1.3x |
| keccak 0.1 simd x8 | 14,119 | 7,062 | 3,535 | 2,829 | 1,770 | 2,831 | 1,771 | 1,774 | 1,776 | 577 | 1.3x |
| own AVX2 x4 | 3,596 | 1,803 | 903 | 1,439 | 880 | 1,065 | 870 | 866 | 906 | 1,131 | 2.5x |
| own AVX-512 x8 | 2,774 | 1,387 | 694 | 555 | 348 | 555 | 348 | 349 | 350 | 2,922 | 6.4x |
| own auto (x8; sha3 for a lone message) | 2,240 | 1,388 | 695 | 556 | 348 | 556 | 348 | 349 | 351 | 2,920 | 6.4x |

### 4 KiB, 1 thread, ns per message

| implementation | 1 | 2 | 4 | 5 | 8 | 10 | 16 | 32 | 64 | MB/s @64 | vs stock @64 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| sha3 0.12 (stock Firewood) | 8,662 | 8,645 | 8,641 | 8,647 | 8,645 | 8,660 | 8,648 | 8,673 | 8,664 | 473 | 1.0x |
| tiny-keccak 2.0 | 8,602 | 8,605 | 8,612 | 8,614 | 8,618 | 8,598 | 8,622 | 8,591 | 8,594 | 477 | 1.0x |
| keccak-asm 0.1 | 7,558 | 7,565 | 7,561 | 7,561 | 7,565 | 7,568 | 7,571 | 7,563 | 7,566 | 541 | 1.1x |
| keccak 0.1 simd x2 | 14,895 | 7,449 | 7,447 | 8,936 | 7,447 | 7,452 | 7,460 | 7,459 | 7,461 | 549 | 1.2x |
| keccak 0.1 simd x4 | 26,238 | 13,126 | 6,563 | 10,499 | 6,562 | 7,877 | 6,574 | 6,574 | 6,575 | 623 | 1.3x |
| keccak 0.1 simd x8 | 54,599 | 27,297 | 13,652 | 10,926 | 6,839 | 10,934 | 6,851 | 6,856 | 6,852 | 598 | 1.3x |
| own AVX2 x4 | 13,318 | 6,659 | 3,373 | 5,330 | 3,345 | 3,997 | 3,343 | 3,338 | 3,335 | 1,228 | 2.6x |
| own AVX-512 x8 | 10,693 | 5,346 | 2,672 | 2,137 | 1,335 | 2,136 | 1,342 | 1,344 | 1,343 | 3,049 | 6.4x |
| own auto (x8; sha3 for a lone message) | 8,648 | 5,346 | 2,672 | 2,139 | 1,334 | 2,138 | 1,342 | 1,343 | 1,344 | 3,048 | 6.4x |

### 16 threads, aggregate MB/s at batch 64 (and at batch 8)

Every implementation scales 15.5x to 16.6x from 1 to 16 threads at batch 64 (aggregate rate over 16 times the single-thread rate), so the AVX-512 kernel shows no frequency penalty on this CPU.

| implementation | 64 B | 136 B | 512 B | 1 KiB | 4 KiB | 64 B @8 | 512 B @8 |
|---|---:|---:|---:|---:|---:|---:|---:|
| sha3 0.12 (stock Firewood) | 3,420 | 3,810 | 7,227 | 7,279 | 7,831 | 3,411 | 7,236 |
| tiny-keccak 2.0 | 3,330 | 3,712 | 7,198 | 7,299 | 7,594 | 3,331 | 7,198 |
| keccak-asm 0.1 | 4,058 | 4,358 | 8,276 | 8,300 | 8,644 | 4,036 | 8,267 |
| keccak 0.1 simd x2 | 4,254 | 4,638 | 8,753 | 8,761 | 9,087 | 4,255 | 8,762 |
| keccak 0.1 simd x4 | 4,712 | 5,089 | 9,574 | 9,614 | 9,959 | 4,714 | 9,610 |
| keccak 0.1 simd x8 | 4,525 | 4,863 | 9,188 | 9,230 | 9,566 | 4,522 | 9,206 |
| own AVX2 x4 | 9,090 | 9,859 | 18,742 | 17,571 | 19,336 | 9,095 | 18,798 |
| own AVX-512 x8 | 21,600 | 24,383 | 46,055 | 46,518 | 48,598 | 21,598 | 46,543 |
| own auto (x8; sha3 for a lone message) | 21,552 | 24,327 | 45,991 | 46,584 | 48,598 | 21,461 | 46,419 |

### `-C target-cpu=native` build, 1 thread, ns per message

Same benchmark built with `RUSTFLAGS="-C target-cpu=native"`. Two rows change a lot: `sha3` 0.12 gets about 2.5x slower (its portable permutation compiles worse with AVX-512 enabled), and "own AVX2 x4" gets 1.7x faster because the compiler now emits AVX-512VL instructions (`vprolq`, `vpternlogq` on 256-bit registers) for the same intrinsics, so it is no longer an AVX2-only kernel. The RustCrypto `simd` rows only use AVX2/AVX-512 in this build; in the generic build `core::simd` lowers them to SSE2.

| implementation | 64 B @1 | 64 B @8 | 64 B @64 | 136 B @64 | 512 B @64 | 1 KiB @64 | 4 KiB @64 | generic build, 512 B @64 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| sha3 0.12 (stock Firewood) | 720 | 718 | 718 | 1,431 | 2,867 | 5,724 | 22,189 | 1,127 |
| tiny-keccak 2.0 | 270 | 269 | 269 | 519 | 1,026 | 2,032 | 7,807 | 1,134 |
| keccak-asm 0.1 | 208 | 208 | 207 | 411 | 815 | 1,629 | 6,300 | 990 |
| keccak 0.1 simd x2 | 281 | 140 | 140 | 274 | 544 | 1,085 | 4,194 | 968 |
| keccak 0.1 simd x4 | 293 | 75.9 | 75.7 | 143 | 283 | 560 | 2,155 | 853 |
| keccak 0.1 simd x8 | 419 | 55.0 | 54.9 | 104 | 206 | 405 | 1,559 | 891 |
| own AVX2 x4 | 264 | 68.1 | 67.0 | 125 | 246 | 483 | 1,850 | 437 |
| own AVX-512 x8 | 364 | 47.9 | 48.0 | 90.2 | 179 | 353 | 1,354 | 178 |
| own auto (x8; sha3 for a lone message) | 721 | 48.0 | 47.9 | 90.3 | 179 | 353 | 1,354 | 178 |


## Takeaways

- With 8 or more messages per call, the own AVX-512 8-lane kernel is 6.3x to 6.4x faster than stock `sha3` at every message size (64 B: 47 ns vs 298 ns per message; 512 B: 176 ns vs 1,127 ns). The AVX2 4-lane kernel is 2.5x to 2.6x faster.
- Lanes are paid for even when idle. One message on the 8-lane kernel costs 363 ns (1.2x slower than stock), but two messages already cost 182 ns each (1.6x faster). The 8-lane kernel beats the 4-lane one at every batch size, even at 4 messages (92 ns vs 113 ns). Batches of 5 or 10 pay for 8 or 16 lanes (74 ns). The "auto" row (8 lanes, a lone message on `sha3`) matches stock at batch 1 and gets the full 8-lane speed from batch 8 up.
- Faster single-message hashing is not the lever: `keccak-asm` is only 1.1x to 1.2x faster than `sha3`, `tiny-keccak` is equal. RustCrypto `keccak` 0.1 `simd` gives only 1.2x to 1.4x in a generic build, needs nightly, and only reaches 55 ns per 64 B message (x8) in a `target-cpu=native` build.
- Lanes and threads multiply: with 16 threads at batch 64 the 8-lane kernel hashes 46 GB/s of 512 B nodes, stock `sha3` 7.2 GB/s.
- No build flags are needed: the own kernels pick AVX-512 or AVX2 at runtime in the normal generic build. Building Firewood with `target-cpu=native` would make stock `sha3` about 2.5x slower on this CPU.

## What a Firewood integration would need

- Collect the dirty nodes per level instead of hashing inside the depth-first recursion: a node can only be hashed after all its children, so the natural order is bottom-up by depth (deepest level first), and every level's nodes are independent.
- Encode each node's RLP first (as `Preimage::write` does today), then hash the level in batches through one call. Nodes whose RLP is shorter than 32 bytes are embedded, not hashed, and stay out of the batch.
- The kernels here need equal-length messages in one call. Real node encodings differ in length, so either group nodes by number of 136-byte blocks (the kernel cost depends only on that, so one group can share the absorb loop and pad each lane separately), or extend the sponge to finish lanes at different blocks. This is the main code change missing from this repo.
- Fall back for small batches: one leftover node goes to the scalar path (`sha3` or `keccak-asm`); two or more already pay off on the 8-lane kernel. On a CPU without AVX-512 use the 4-lane AVX2 kernel; without AVX2 use the scalar path. `is_x86_feature_detected!` makes this a runtime choice, no build flags.
- Keep the per-subtrie workers of `ParallelMerkle`; lanes speed up each worker and threads scale on top (see the 16-thread table).
- Do not build Firewood with `-C target-cpu=native` without measuring: in that build `sha3` 0.12 gets about 2.5x slower on this CPU (table above).

## Limits

- x86 only (one Xeon 6975P-C, one c8i.8xlarge in AWS, a shared host). Nothing here covers ARM (Graviton, Apple), where AVX-512 does not exist and the trade-off is different.
- Equal-length batches only, and synthetic random messages, not real trie nodes. The tables measure hashing in isolation: no RLP encoding, no trie walk, no allocation.
- Single-thread numbers of the own kernels moved by up to about 5% between runs and CPUs after the state and last block were cache-line aligned (before that fix, by up to 20%). Treat differences under 5% as noise; the scalar rows repeated within 1%.

## How to run

```sh
cargo test --release                       # vectors + 3 s randomized test (FUZZ_SECS=60 for longer)
cargo bench                                # all stable implementations, 1 and 16 threads
cargo +nightly bench --features rustcrypto-simd     # adds the keccak 0.1 simd rows (this is what the tables use)
BENCH_THREADS=1 BENCH_IMPL="own" cargo bench        # filter by thread count and implementation name
RUSTFLAGS="-C target-cpu=native" cargo +nightly bench --features rustcrypto-simd   # native-build table
```

`keccak-asm` needs perl and a C toolchain (`apt-get install -y build-essential perl`). The bench prints CSV rows and then Markdown tables; the raw output behind this README is in `results/` (`generic-build.txt`, `target-cpu-native-build.txt`; the auto row label had a comma at run time and was changed to a semicolon in the files). The 16-thread mode pins thread t to CPU t, so on a machine where CPUs 0 to 15 are not separate physical cores, pin or read the numbers accordingly.

## Firewood A/B on the CS shaped workload

The [Firewood patch](firewood/lanes-hash-v0.8.0.patch) applies to v0.8.0. [HASHING.md](firewood/HASHING.md) explains the modes, build commands, and root tests; [csbench](firewood/csbench/) is the standalone Go harness. This experiment extends the kernel benchmark above to real trie nodes with variable-length encodings.

One contract has N storage slots. Each block reads D distinct random slots, writes new values to them, proposes the state, then commits. The database uses pruning and deferred persistence. Database close is outside the block timer, so commit is not a per-block fsync measurement. The state fits in the page cache at the recorded residency snapshots; residency throughout every run is UNVERIFIED. `db` calls Firewood through the Go FFI. `e2e` uses libevm StateDB and Firewood TrieDB, the storage path a stateful precompile takes, without transaction execution, gas, or bytecode.

The reported box is c8i.8xlarge (instance type UNVERIFIED by metadata), with an Intel Xeon 6975P-C and AVX-512. Runs used `GOMAXPROCS=16`, `RAYON_NUM_THREADS=16`, `taskset -c 0-15`, revisions 32, seed 1, 5 warmup blocks and 20 measured blocks. Commands and hardware evidence are in [COMMANDS.md](firewood/results/c8i-8xlarge/COMMANDS.md), [jobs.txt](firewood/results/c8i-8xlarge/jobs.txt), and [REPORT.md](firewood/results/c8i-8xlarge/REPORT.md). Measurements ran on 2026-10-14 JST.

`FIREWOOD_HASH_MODE=stock` selects the original scalar hash path in the patched library. `lanes` batches independent nodes by depth through the SIMD kernel. `par` splits hashing inside one hot account subtrie across Rayon tasks. `lanes+par` combines them. `bundled` is the prebuilt Go module library, kept as a separate baseline. Account special cases stay on the stock path. The SIMD kernels are x86-only, with scalar fallback elsewhere.

### Per-block timings

All values are milliseconds except the final column. Each repeat takes the median of each field over measured blocks; the table takes the mean of those medians. Total is measured separately and is not the sum of phase medians. The table is reproduced exactly by `python3 firewood/results/c8i-8xlarge/summarize.py`; raw commands for every cell are in `jobs.txt` and `run-cell.sh` beside it.

| N | D | cache MiB | level | mode | repeats | read ms | write ms | propose ms | commit ms | total ms | us per dirty slot |
|---:|---:|---:|:---|:---|---:|---:|---:|---:|---:|---:|---:|
| 2000000 | 9600 | 1024 | db | bundled | 2 | 50.560 | 4.661 | 122.391 | 0.498 | 177.873 | 18.528 |
| 2000000 | 9600 | 1024 | db | lanes | 2 | 52.233 | 4.468 | 111.960 | 0.456 | 173.199 | 18.042 |
| 2000000 | 9600 | 1024 | db | lanes+par | 2 | 53.175 | 4.557 | 89.814 | 0.504 | 187.056 | 19.485 |
| 2000000 | 9600 | 1024 | db | par | 2 | 51.994 | 4.660 | 93.949 | 0.530 | 193.772 | 20.185 |
| 2000000 | 9600 | 1024 | db | stock | 2 | 52.634 | 4.981 | 120.426 | 0.470 | 178.358 | 18.579 |
| 2000000 | 9600 | 1024 | e2e | bundled | 2 | 52.298 | 3.899 | 142.391 | 0.554 | 204.892 | 21.343 |
| 2000000 | 9600 | 1024 | e2e | lanes | 2 | 53.779 | 3.731 | 132.294 | 0.476 | 192.753 | 20.078 |
| 2000000 | 9600 | 1024 | e2e | lanes+par | 2 | 52.845 | 3.779 | 103.227 | 0.535 | 160.246 | 16.692 |
| 2000000 | 9600 | 1024 | e2e | par | 2 | 52.494 | 3.764 | 105.569 | 0.501 | 161.557 | 16.829 |
| 2000000 | 9600 | 1024 | e2e | stock | 2 | 53.710 | 3.749 | 141.438 | 0.497 | 200.039 | 20.837 |
| 2000000 | 38400 | 256 | db | bundled | 1 | 208.995 | 20.466 | 409.789 | 1.848 | 652.453 | 16.991 |
| 2000000 | 38400 | 256 | db | lanes | 1 | 232.453 | 20.570 | 379.876 | 1.595 | 637.417 | 16.599 |
| 2000000 | 38400 | 256 | db | lanes+par | 1 | 228.194 | 20.871 | 263.893 | 1.641 | 514.849 | 13.408 |
| 2000000 | 38400 | 256 | db | par | 1 | 230.306 | 20.534 | 265.380 | 1.666 | 522.962 | 13.619 |
| 2000000 | 38400 | 256 | db | stock | 1 | 229.870 | 20.803 | 403.776 | 1.627 | 657.485 | 17.122 |
| 2000000 | 38400 | 1024 | db | bundled | 2 | 222.667 | 20.555 | 433.261 | 1.993 | 687.823 | 17.912 |
| 2000000 | 38400 | 1024 | db | lanes | 2 | 244.572 | 21.020 | 425.449 | 1.803 | 686.212 | 17.870 |
| 2000000 | 38400 | 1024 | db | lanes+par | 2 | 237.021 | 20.626 | 306.519 | 1.843 | 556.031 | 14.480 |
| 2000000 | 38400 | 1024 | db | par | 2 | 241.741 | 21.085 | 309.081 | 1.821 | 571.054 | 14.871 |
| 2000000 | 38400 | 1024 | db | stock | 2 | 246.821 | 21.045 | 447.322 | 1.770 | 716.066 | 18.648 |
| 2000000 | 38400 | 1024 | e2e | bundled | 2 | 232.831 | 16.400 | 536.684 | 2.114 | 791.657 | 20.616 |
| 2000000 | 38400 | 1024 | e2e | lanes | 2 | 231.238 | 16.716 | 529.066 | 1.832 | 791.091 | 20.601 |
| 2000000 | 38400 | 1024 | e2e | lanes+par | 2 | 245.126 | 16.554 | 409.348 | 1.846 | 671.756 | 17.494 |
| 2000000 | 38400 | 1024 | e2e | par | 2 | 247.655 | 16.987 | 416.970 | 1.809 | 681.220 | 17.740 |
| 2000000 | 38400 | 1024 | e2e | stock | 2 | 236.622 | 16.851 | 552.952 | 1.836 | 825.113 | 21.487 |
| 6000000 | 38400 | 1024 | db | bundled | 2 | 310.523 | 21.398 | 545.051 | 2.458 | 880.703 | 22.935 |
| 6000000 | 38400 | 1024 | db | lanes | 2 | 334.684 | 21.493 | 517.030 | 2.148 | 863.263 | 22.481 |
| 6000000 | 38400 | 1024 | db | lanes+par | 2 | 322.201 | 21.347 | 378.731 | 2.246 | 719.120 | 18.727 |
| 6000000 | 38400 | 1024 | db | par | 2 | 315.728 | 20.657 | 381.361 | 2.268 | 714.138 | 18.597 |
| 6000000 | 38400 | 1024 | db | stock | 2 | 327.179 | 21.601 | 548.898 | 2.127 | 879.607 | 22.906 |
| 6000000 | 38400 | 1024 | e2e | bundled | 2 | 334.048 | 16.029 | 635.285 | 2.428 | 972.472 | 25.325 |
| 6000000 | 38400 | 1024 | e2e | lanes | 2 | 330.686 | 16.119 | 611.557 | 2.109 | 956.955 | 24.921 |
| 6000000 | 38400 | 1024 | e2e | lanes+par | 2 | 342.598 | 15.732 | 495.768 | 2.218 | 842.155 | 21.931 |
| 6000000 | 38400 | 1024 | e2e | par | 2 | 339.038 | 15.967 | 501.198 | 2.249 | 842.672 | 21.945 |
| 6000000 | 38400 | 1024 | e2e | stock | 2 | 330.960 | 16.084 | 646.283 | 2.211 | 996.887 | 25.961 |

### Change from patched stock

Positive reduction means less time. Speedup is stock time divided by mode time. These values come from `python3 firewood/results/c8i-8xlarge/gains.py`, using unrounded repeat summaries.

| N | D | cache MiB | level | mode | propose speedup | propose reduction | total speedup | total reduction |
|---:|---:|---:|:---|:---|---:|---:|---:|---:|
| 2000000 | 9600 | 1024 | db | bundled | 0.984x | -1.6% | 1.003x | 0.3% |
| 2000000 | 9600 | 1024 | db | lanes | 1.076x | 7.0% | 1.030x | 2.9% |
| 2000000 | 9600 | 1024 | db | par | 1.282x | 22.0% | 0.920x | -8.6% |
| 2000000 | 9600 | 1024 | db | lanes+par | 1.341x | 25.4% | 0.953x | -4.9% |
| 2000000 | 9600 | 1024 | e2e | bundled | 0.993x | -0.7% | 0.976x | -2.4% |
| 2000000 | 9600 | 1024 | e2e | lanes | 1.069x | 6.5% | 1.038x | 3.6% |
| 2000000 | 9600 | 1024 | e2e | par | 1.340x | 25.4% | 1.238x | 19.2% |
| 2000000 | 9600 | 1024 | e2e | lanes+par | 1.370x | 27.0% | 1.248x | 19.9% |
| 2000000 | 38400 | 256 | db | bundled | 0.985x | -1.5% | 1.008x | 0.8% |
| 2000000 | 38400 | 256 | db | lanes | 1.063x | 5.9% | 1.031x | 3.1% |
| 2000000 | 38400 | 256 | db | par | 1.521x | 34.3% | 1.257x | 20.5% |
| 2000000 | 38400 | 256 | db | lanes+par | 1.530x | 34.6% | 1.277x | 21.7% |
| 2000000 | 38400 | 1024 | db | bundled | 1.032x | 3.1% | 1.041x | 3.9% |
| 2000000 | 38400 | 1024 | db | lanes | 1.051x | 4.9% | 1.044x | 4.2% |
| 2000000 | 38400 | 1024 | db | par | 1.447x | 30.9% | 1.254x | 20.3% |
| 2000000 | 38400 | 1024 | db | lanes+par | 1.459x | 31.5% | 1.288x | 22.3% |
| 2000000 | 38400 | 1024 | e2e | bundled | 1.030x | 2.9% | 1.042x | 4.1% |
| 2000000 | 38400 | 1024 | e2e | lanes | 1.045x | 4.3% | 1.043x | 4.1% |
| 2000000 | 38400 | 1024 | e2e | par | 1.326x | 24.6% | 1.211x | 17.4% |
| 2000000 | 38400 | 1024 | e2e | lanes+par | 1.351x | 26.0% | 1.228x | 18.6% |
| 6000000 | 38400 | 1024 | db | bundled | 1.007x | 0.7% | 0.999x | -0.1% |
| 6000000 | 38400 | 1024 | db | lanes | 1.062x | 5.8% | 1.019x | 1.9% |
| 6000000 | 38400 | 1024 | db | par | 1.439x | 30.5% | 1.232x | 18.8% |
| 6000000 | 38400 | 1024 | db | lanes+par | 1.449x | 31.0% | 1.223x | 18.2% |
| 6000000 | 38400 | 1024 | e2e | bundled | 1.017x | 1.7% | 1.025x | 2.4% |
| 6000000 | 38400 | 1024 | e2e | lanes | 1.057x | 5.4% | 1.042x | 4.0% |
| 6000000 | 38400 | 1024 | e2e | par | 1.289x | 22.4% | 1.183x | 15.5% |
| 6000000 | 38400 | 1024 | e2e | lanes+par | 1.304x | 23.3% | 1.184x | 15.5% |

At N=2M, D=9600, `par` and `lanes+par` improve propose time but make the db total worse. The blocks are bimodal; the cause is UNVERIFIED. The e2e result improves on the same workload. Do not infer total time by adding the phase medians. The cache 256 comparison has only one repeat, and no confidence intervals were computed. N=6M, D=9600 was not measured.

### Where the CPU time goes

Profiles cover db at N=2M, D=38400, cache 1024, measured blocks 6-25. These are whole-process CPU sample shares across all threads, not percentages of propose wall time. `profile.sh` records the capture commands. The commands below produced the saved attribution tables; their large perf data and dwarf text inputs are excluded from this repository. Repeating the capture is required to regenerate those inputs.

```sh
python3 profile-classify.py stock raw/profile-stock.report raw/profile-stock-dwarf.script > profile-stock.txt
python3 profile-classify.py lanes+par raw/profile-lanes+par.report raw/profile-lanes+par-dwarf.script > profile-lanes+par.txt
```

| bucket | stock | lanes+par |
|:---|---:|---:|
| keccak (Firewood, Rust) | 8.11% | 2.52% |
| keccak (Go side) | 2.41% | 2.79% |
| hash traversal (non-keccak) | 12.21% | 13.78% |
| pread/pwrite + page cache | 8.07% | 8.50% |
| background persist worker (non-I/O) | 22.19% | 22.36% |
| read_for_update/insert (propose) | 18.10% | 19.36% |
| node clone, caller truncated by 8 KiB dwarf stack | 9.41% | 9.84% |
| get/read path | 13.84% | 13.80% |
| Go runtime | 1.71% | 1.76% |
| other Go | 0.84% | 0.86% |
| kernel other | 0.56% | 1.02% |
| other/unattributed | 2.54% | 3.40% |

Top self-time symbols in stock include `Box<BranchNode>::clone` at 10.35%, Rust `keccak_p` at 8.29%, and committed `MaybePersistedNode::as_shared_node` at 4.15%. In `lanes+par`, the top lines are `Box<BranchNode>::clone` at 11.16%, `drop_glue<Option<Child>>` at 4.42%, and committed `MaybePersistedNode::as_shared_node` at 3.84%. The same classification commands produce these lines in [profile-stock.txt](firewood/results/c8i-8xlarge/profile-stock.txt) and [profile-lanes+par.txt](firewood/results/c8i-8xlarge/profile-lanes+par.txt). Attribution uses symbol rules and is approximate. Callers beyond truncated stacks are UNVERIFIED; e2e CPU attribution is UNVERIFIED because it was not profiled.

All recorded timing runs match state roots, storage roots, and read checksums across modes and levels for the same workload (`python3 firewood/results/c8i-8xlarge/validate-all.py`). The standalone harness passed `go vet ./...` and `go run . -n 20000 -d 500 -blocks 2 -warmup 1 -self-check` on the Mac, including cross-mode root checks with the patched library. [Packaging verification](firewood/results/VERIFICATION.md) records the commands. Standalone c8i timing after packaging is UNVERIFIED; the measured block code is unchanged from the original harness.

Lanes alone saves a few percent on this path because Keccak is only a small part of the work, and batching still requires trie traversal and encoding. Parallel hashing inside the hot account subtrie gives the larger gain. The remaining work includes reads through the FFI, serial trie updates and node cloning, and background persistence. This supports parallel hash traversal as the larger opportunity here, with the small-workload db regression above as an important limit.

## License

Copyright (c) 2026 Ava Labs, Inc. Open source but not free: you may use, modify and distribute this code only as part of software that runs on, or serves, the Avalanche Network and Avalanche L1s; any other use requires written permission from Ava Labs, Inc. Provided as is, without warranty. See `LICENSE`.
