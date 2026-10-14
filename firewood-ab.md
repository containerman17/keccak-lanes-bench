# Firewood A/B on the CS shaped workload

The [Firewood patch](firewood/lanes-hash-v0.8.0.patch) applies to v0.8.0. [HASHING.md](firewood/HASHING.md) explains the modes, build commands, and root tests; [csbench](firewood/csbench/) is the standalone Go harness. This experiment extends the [kernel benchmark](kernel-benchmark.md) to real trie nodes with variable-length encodings.

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

