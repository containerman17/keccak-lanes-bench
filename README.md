# keccak-lanes-bench

Firewood hashes trie nodes one at a time with the portable `sha3` crate. Nodes on the same trie level are independent, so an AVX2 or AVX-512 kernel can hash 4 or 8 of them in one permutation pass. This repo measures (1) what that kernel is worth on x86 and (2) what it is worth inside Firewood on a write-heavy, one-hot-contract workload with the state in the page cache.

## Findings

- Kernel alone: the own AVX-512 8-lane kernel is 6.3x faster than stock `sha3` 0.12 from 8 messages per call up, at every message size (64 B: 47 ns vs 298 ns per message). AVX2 4-lane: 2.5x. Faster single-message crates (`keccak-asm`) give only 1.1x to 1.2x. Details: [kernel-benchmark.md](kernel-benchmark.md).
- Inside Firewood (v0.8.0, c8i.8xlarge, one contract, 2M slots, 38,400 dirty slots per block, through the Go FFI): SIMD Keccak alone (`lanes`) cuts the block by 4%, because Keccak is only 8% of the process CPU on this path. Hashing the hot contract's subtrie on several threads (`par`) cuts the block by 20% (db level) and 17% (libevm StateDB e2e); both together 22% and 19%. At 6M slots: 18% and 16%. Details: [firewood-ab.md](firewood-ab.md).
- Where the rest goes (current Firewood, db level): `Box<BranchNode>::clone` 10%, persist worker 22%, `read_for_update`/insert 18%, reads through the FFI about 6 us each (a third of the block).

| Firewood v0.8.0, one hot contract, 2M slots, 38,400 dirty slots per block | db propose ms | db block ms | vs current | e2e block ms | vs current |
|:---|---:|---:|---:|---:|---:|
| current Firewood (one Keccak per node, one thread per hot contract) | 447 | 716 | 0% | 825 | 0% |
| SIMD Keccak, 8 nodes per call (`lanes`) | 425 | 686 | -4% | 791 | -4% |
| parallel hashing inside the hot contract (`par`) | 309 | 571 | -20% | 681 | -17% |
| both (`lanes+par`) | 307 | 556 | -22% | 672 | -19% |

All modes produce identical roots in every cell.

## Layout

- `src/lanes.rs`, `benches/lanes.rs`, `results/`: the kernels and the kernel benchmark ([kernel-benchmark.md](kernel-benchmark.md)).
- `firewood/lanes-hash-v0.8.0.patch`, `firewood/HASHING.md`: the Firewood patch (modes `FIREWOOD_HASH_MODE=stock|lanes|par|lanes+par`), build commands and root tests.
- `firewood/csbench/`: standalone Go harness (db level = Firewood through the FFI, e2e = libevm StateDB + Firewood TrieDB).
- `firewood/results/c8i-8xlarge/`: raw outputs, matrix, profiles, commands ([firewood-ab.md](firewood-ab.md)).

## How to run

```sh
cargo test --release           # kernel vectors + randomized test against tiny-keccak
cargo bench                    # kernel tables, 1 and 16 threads
```

The Firewood A/B build and run commands are in [firewood/HASHING.md](firewood/HASHING.md) and [firewood/csbench/README.md](firewood/csbench/README.md).

## License

Copyright (c) 2026 Ava Labs, Inc. Open source but not free: you may use, modify and distribute this code only as part of software that runs on, or serves, the Avalanche Network and Avalanche L1s; any other use requires written permission from Ava Labs, Inc. Provided as is, without warranty. See `LICENSE`.
