# c8i Firewood hashing benchmark (CS shaped workload)

Generated 2026-10-14 JST. Measurements ran 2026-10-14 between about 12:00 and 13:40 JST.

## Box

- AWS instance type c8i.8xlarge (from the brief; the metadata call returned nothing, see UNVERIFIED). CPU: Intel Xeon 6975P-C, 16 cores, 2 threads per core, 32 vCPU. vCPU 0-15 are 16 different physical cores (`lscpu -e`, in build-identity.txt). AVX-512 present (avx512f, bw, dq, vl, vbmi, ifma, vnni, bitalg, vpopcntdq).
- RAM: MemTotal 64765964 kB (61.8 GiB). No swap.
- Disk: 40 GB EBS (nvme0n1, "Amazon Elastic Block Store"), 38 GB root fs.
- OS: Ubuntu 24.04.5 LTS, kernel 7.0.0-1014-aws, THP madvise.
- Go 1.26.8, rustc 1.99.0, perf from linux-tools 7.0.0-1014-aws.
- Benchmark host IP: 54.215.196.23.

## What was measured

- Harness: `bench/go/csbench` (copy of ~/temp-1m-clob on the box at ~/csbench, sha256 in build-identity.txt). One account, N storage slots; each block reads D random distinct slots, writes them, then propose and commit. See bench/go/csbench/README.md.
- Levels: `db` is direct Firewood FFI (Get, Propose, Commit). `e2e` goes through the libevm StateDB adapter (GetState, SetState, Finalise, IntermediateRoot, Commit).
- Modes: `bundled` = prebuilt lib inside Go module github.com/ava-labs/firewood-go-ethhash/ffi v0.8.0 (no CGO_LDFLAGS). `stock`, `lanes`, `par`, `lanes+par` = the modified Firewood v0.8.0 worktree (~/firewood, from ~/.herdr/worktrees/firewood/lanes-hash) linked with CGO_LDFLAGS, mode chosen at runtime by FIREWOOD_HASH_MODE. `stock` in the modified lib = original keccak and serial hashing (code path chosen in storage/src/hashers/batched.rs). `lanes` = 8-way AVX-512 keccak. `par` = parallel subtrie hashing. `lanes+par` = both.
- Settings for every cell: GOMAXPROCS=16, RAYON_NUM_THREADS=16, `taskset -c 0-15`, revisions 32, seed 1, 5 warmup blocks + 20 measured blocks. Cache 1024 MiB unless the row says 256.
- Cells named `gate-<mode>` are repeat 1 of N=2M D=38400 (they used -self-check, like all 1024 MiB cells). With `-self-check` one process runs the db level and then the e2e level on fresh DBs, so one cell gives both levels.
- Statistic: for each cell and level, the median of each phase over the 20 measured blocks; then the mean of these medians over repeats. total is the median of per-block totals (not the sum of phase medians). us per dirty slot = per-block total_ms * 1000 / D, same statistic.
- Correctness: every cell of the same (N, D) produced identical state roots, storage roots, and read checksums for the seed and all 25 blocks, across modes and both levels (all-roots.txt, correctness.txt). The new cache 256 cells and the 4 profile runs also match the gate-stock db sequence (checked 2026-10-14 about 13:40 JST).

## Exact commands

Remote prefix: `ssh -i ~/.ssh/claude -o IdentitiesOnly=yes ubuntu@54.215.196.23 '<cmd>'`. Setup, transfer and build: COMMANDS.md and setup.sh.

One cell (run-cell.sh, in this folder):

```sh
nohup bash ~/run-cell.sh <name> <mode> db <N> <D> <cacheMiB> 20 5 [-self-check] > ~/<name>-launch.log 2>&1 < /dev/null &
# inside run-cell.sh:
export GOMAXPROCS=16 RAYON_NUM_THREADS=16 FIREWOOD_HASH_MODE=$mode
[ $mode != bundled ] && export CGO_LDFLAGS=-L/home/ubuntu/firewood/target/x86_64-unknown-linux-gnu/release
taskset -c 0-15 go run ./bench/go/csbench -level db -n $N -d $D -cache-mb $cache -revisions 32 -seed 1 -blocks 20 -warmup 5 -datadir /home/ubuntu/dbs -print-seed-root -json [-self-check]
```

Every cell with its arguments is in jobs.txt. Cells ran one at a time, nothing else on the box.

Profiles (profile.sh, in this folder): go run with `-ldflags="-s=false -w=false"` (unstripped), N=2M D=38400 cache 1024 db level, attach after block 5 (end of warmup), stop after block 25:

```sh
nohup bash ~/profile.sh stock > ~/results/profile-stock.launch 2>&1 < /dev/null &          # perf record -e cpu-clock -F 499 -g -p PID
nohup bash ~/profile.sh lanes+par > ~/results/profile-lanes+par.launch 2>&1 < /dev/null &
SUFFIX=-dwarf CG="--call-graph dwarf,8192" nohup bash ~/profile.sh stock ...               # same, dwarf unwinding, for attribution
SUFFIX=-dwarf CG="--call-graph dwarf,8192" nohup bash ~/profile.sh lanes+par ...
perf report -i profile-<mode>.data --stdio --no-children --sort symbol --call-graph none --percent-limit 0 > profile-<mode>.report
perf report -i profile-<mode>.data --stdio --no-children --percent-limit 0.8 > profile-<mode>.callgraph
for t in <tids>; do perf script -f -i profile-<mode>-dwarf.data --no-inline -F comm,tid,ip,sym,dso --tid $t; done | sed -E 's/ \([^()]*\)$//' > profile-<mode>-dwarf.script
python3 profile-classify.py <mode> raw/profile-<mode>.report raw/profile-<mode>-dwarf.script > profile-<mode>.txt
```

The event is cpu-clock (software): the earlier capture used it in perf-probe.txt; hardware cycles were not tried. Older profiles at -F 199 are kept in raw/profile-f199/ (plus raw/profile-stripped and raw/profile-including-close, both excluded).

Analysis (on the Mac, in this folder):

```sh
rsync -az -e 'ssh -i ~/.ssh/claude -o IdentitiesOnly=yes' ubuntu@54.215.196.23:results/ raw/
python3 summarize.py > matrix.md      # also writes summary.json
python3 gains.py > gains.txt
python3 repeats.py > repeats.csv      # per-repeat medians, min and max
python3 validate-all.py > all-roots.txt
```

## Matrix

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

## Percentages vs modified stock

speedup = stock ms / mode ms. reduction = 1 - mode ms / stock ms. Positive reduction = faster than stock. `bundled` vs stock shows the build difference between the Go module's prebuilt lib and our build of the modified lib in stock mode.

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

### us per dirty slot (per-block total, median)

| N | D | cache MiB | level | stock us/dirty slot | lanes+par us/dirty slot | bundled us/dirty slot |
|---:|---:|---:|:---|---:|---:|---:|
| 2000000 | 9600 | 1024 | db | 18.58 | 19.49 | 18.53 |
| 2000000 | 9600 | 1024 | e2e | 20.84 | 16.69 | 21.34 |
| 2000000 | 38400 | 256 | db | 17.12 | 13.41 | 16.99 |
| 2000000 | 38400 | 1024 | db | 18.65 | 14.48 | 17.91 |
| 2000000 | 38400 | 1024 | e2e | 21.49 | 17.49 | 20.62 |
| 6000000 | 38400 | 1024 | db | 22.91 | 18.73 | 22.93 |
| 6000000 | 38400 | 1024 | e2e | 25.96 | 21.93 | 25.32 |

### Notes on the numbers

- N=2M D=9600 db: par and lanes+par cut the propose median by 22-25%, but the total median is 5-9% worse than stock. Per-block data is bimodal: in run-par-n2-d9600-r1 the fast blocks take 120-140 ms total, but about half the blocks have a propose of 140-245 ms or a read spike of 110-510 ms (repeats.csv: min 118 ms, max 611 ms). The 20-block median falls on the slow group. The e2e level at the same N and D shows a 19-20% total gain. The cause of the slow blocks is not known (UNVERIFIED guess: contention with the background persist worker).
- At D=38400 the two repeats agree within a few percent (repeats.csv), and the cache 256 row (1 repeat) shows the same picture as cache 1024.
- `lanes` alone gives 5-7% on propose and 2-4% on total. Almost all of the gain comes from `par`; `lanes` on top of `par` adds 0-1.5 points.

## Profiles (db level, N=2M D=38400, cache 1024, measured blocks 6-25)

During the profiled runs the propose median was 409.7 ms (stock) and 279.4 ms (lanes+par); totals 653.0 and 513.5 ms. Percentages below are shares of CPU samples of the whole process (all threads), not of wall time. Full top 30 lists: profile-stock.txt, profile-lanes+par.txt; call graphs: raw/profile-<mode>.callgraph.

Self-time keccak (from the -g -F 499 run, symbols with keccak/sha3/lanes::x86): stock: Rust/Firewood 8.58%, Go (csbench key/slot hashing) 2.44%. lanes+par: Rust/Firewood 2.53%, Go (csbench key/slot hashing) 2.50%. Rust keccak symbols are `keccak::backends::soft::keccak_p` (stock) and `keccak_p` + `hashers::lanes::x86::kernel8` (lanes+par). Go keccak is `golang.org/x/crypto/sha3.keccakF1600` (csbench key hashing, outside Firewood).

Call-chain attribution (dwarf run, first matching rule wins, rules in profile-classify.py):

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

How to read the buckets: "hash traversal" = samples under hash_helper / hashers::batched / children_hashes / HashableShunt / ethhash that are not keccak itself (node walk, RLP preimage building, Children::map). "pread/pwrite + page cache" = any frame in pread, pwrite, vfs_read/write, filemap. "background persist worker" = the PersistLoop thread (serialize + free list + allocation), not counting its pwrite. "read_for_update/insert" = Merkle insert_helper, read_for_update (copy-on-write of branch nodes; most `Box<BranchNode>::clone` samples have read_for_update as caller), ParallelMerkle workers. "node clone, caller truncated" = recursive Box<BranchNode>/Child clone chains deeper than the 8 KiB dwarf stack copy; where the stock chain does reach a non-clone Firewood caller, it is read_for_update in 639 samples and hash_helper_inner in 6, so these are most likely read_for_update too (UNVERIFIED). "get/read path" = Firewood get (fwd_get_latest, get_helper, read_cached_node, LRU insert) without pread.

Top 12 self symbols, stock:

| self | symbol |
|---:|:---|
| 10.35% | `<Box<node::branch::BranchNode> as core::clone::Clone>::clone` |
| 8.29% | `keccak::backends::soft::keccak_p::<u64, 24>` |
| 4.15% | `<node::persist::MaybePersistedNode>::as_shared_node::<&nodestore::NodeStore<nodestore::Committed, FileBacked` |
| 3.98% | `drop_glue::<Option<node::branch::Child>>` |
| 3.28% | `<node::branch::Child as core::clone::Clone>::clone` |
| 3.00% | `<node::path::Path as core::ops::deref::Deref>::deref` |
| 2.35% | `golang.org/x/crypto/sha3.keccakF1600` |
| 2.22% | `<lru_mem::LruCache<nodestore::primitives::LinearAddress, CachedNode>>::insert` |
| 2.16% | `<FileBacked as linear::ReadableStorage>::read_cached_node` |
| 2.09% | `<metrics_util::registry::Registry<metrics::key::Key, metrics_util::registry::recency::GenerationalStorage<metr` |
| 2.07% | `drop_glue::<node::Node>` |
| 2.06% | `<node::persist::MaybePersistedNode>::as_shared_node::<nodestore::NodeStore<nodestore::Mutable<nodestore::Propo` |

Top 12 self symbols, lanes+par:

| self | symbol |
|---:|:---|
| 11.16% | `<Box<node::branch::BranchNode> as core::clone::Clone>::clone` |
| 4.42% | `drop_glue::<Option<node::branch::Child>>` |
| 3.84% | `<node::persist::MaybePersistedNode>::as_shared_node::<&nodestore::NodeStore<nodestore::Committed, FileBacked` |
| 3.66% | `<node::path::Path as core::ops::deref::Deref>::deref` |
| 3.57% | `<node::branch::Child as core::clone::Clone>::clone` |
| 2.39% | `<lru_mem::LruCache<nodestore::primitives::LinearAddress, CachedNode>>::insert` |
| 2.24% | `golang.org/x/crypto/sha3.keccakF1600` |
| 2.17% | `<FileBacked as linear::ReadableStorage>::read_cached_node` |
| 2.16% | `drop_glue::<node::Node>` |
| 2.15% | `<node::persist::MaybePersistedNode>::as_shared_node::<nodestore::NodeStore<nodestore::Mutable<nodestore::Propo` |
| 2.14% | `<[&Option<node::branch::Child>; 16]>::try_map::<core::ops::try_trait::NeverShortCircuit<Option<hashtype::ethha` |
| 2.00% | `<metrics_util::registry::Registry<metrics::key::Key, metrics_util::registry::recency::GenerationalStorage<metr` |

Summary: in stock, Firewood keccak is about 8% of process CPU and non-keccak hash traversal about 12%. lanes+par cuts Firewood keccak to about 2.5%; traversal stays at 12-14% (now spread over rayon threads). The rest of the CPU is node copy-on-write (read_for_update, Box<BranchNode>::clone, drop of Child), the read path, page-cache memcpy, the persist worker, and a metrics counter registry lookup (about 2% self). Keccak was never the dominant cost; the gain of `par` comes from running the hash traversal in parallel, not from a faster permutation.

## Residency evidence

residency-2m.txt and residency-6m.txt: residency.py maps the Firewood DB file and calls mincore during measured blocks.

- N=2M (2026-10-14 12:22:03 JST): firewood.db 458446298 bytes, 111926 of 111926 pages resident. MemAvailable 56929680 kB.
- N=6M (2026-10-14 12:27:57 JST, bundled r1): firewood.db 1253234928 bytes, 305966 of 305966 pages resident. MemAvailable 52705584 kB.

So the whole DB file was in the page cache at those moments. pread costs here are page-cache memcpy, not EBS reads. This is a snapshot, not proof for every instant of every run.

## Inventory

All cells have exit 0 (raw/*.exit). Present: 5 modes x {2M D38400 (gate = r1, r2), 6M D38400 (r1, r2), 2M D9600 (r1, r2)} with both levels each, and cache 256 db cells for all 5 modes at 2M D38400 (1 repeat). par and lanes+par cache 256 ran at 13:24-13:26 JST. No expected cell is missing. link-bad (exit 1) is the intended negative test that a broken lib fails the link; it is not a measurement. ~/dbs on the box is empty.

## UNVERIFIED

- Instance type c8i.8xlarge: from the brief; the instance metadata call returned nothing. The CPU model (Xeon 6975P-C, 16 cores x 2 threads) is consistent with it.
- The cause of the bimodal block times at D=9600 db (persist worker contention is a guess).
- The callers of the truncated deep-clone chains (assumed read_for_update).
- That `bundled` (firewood-go-ethhash/ffi v0.8.0 prebuilt) is built from the same Firewood source as our v0.8.0 worktree; only the speed difference (stock vs bundled within -4% to +2.5% total) was measured.
- Profile attribution rules are regex heuristics on symbol names; buckets are approximate. Sampling with cpu-clock at 499 Hz gives about 8.5k samples per dwarf run; bucket shares have roughly +-1 point sampling error.
- e2e level was not profiled.
- Residency was checked at one moment per N, not during every run. The cache 256 cells did not get a residency snapshot.
- Run-to-run noise: only 2 repeats (1 for cache 256); no confidence intervals.
- Earlier steps completed before 13:22 JST (setup, build, gates, cells, residency snapshots, build identity) were not rerun; their logs and outputs are in this folder and raw/.

## Published artifact scope

This copy omits perf binary data and dwarf script output. Text profile reports,
call graphs, classifications, raw JSONL, stdout, exit status, memory snapshots,
and residency evidence are retained. Commands preserve the original capture
paths. For the standalone module, run from `firewood/csbench` and replace
`go run ./bench/go/csbench` with `go run .`.
