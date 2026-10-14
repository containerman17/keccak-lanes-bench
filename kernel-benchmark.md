# Keccak-256 lanes: kernel benchmark

Details behind the kernel numbers in the [README](README.md). Everything here is hashing in isolation on synthetic messages; the Firewood measurement is in [firewood-ab.md](firewood-ab.md).

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

