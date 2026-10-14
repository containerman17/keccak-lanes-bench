# c8i commands

All timestamps use JST. Remote shell prefix:

```sh
ssh -i ~/.ssh/claude -o IdentitiesOnly=yes ubuntu@54.215.196.23 '<command>'
```

Setup: uploaded `setup.sh`, then `nohup bash ~/setup.sh > ~/setup.log 2>&1 < /dev/null &`.
Source transfer:

```sh
rsync -az -e 'ssh -i /Users/ilia/.ssh/claude -o IdentitiesOnly=yes' --exclude target --exclude .git --exclude evidence /Users/ilia/.herdr/worktrees/firewood/lanes-hash/ ubuntu@54.215.196.23:firewood/
rsync -az -e 'ssh -i /Users/ilia/.ssh/claude -o IdentitiesOnly=yes' --include '*/' --include '*.go' --include 'go.mod' --include 'go.sum' --include '*.h' --include '*.c' --exclude '*' /Users/ilia/temp-1m-clob/ ubuntu@54.215.196.23:csbench/
```

An initial broad transfer was stopped because unrelated artifacts were large. The source transfer above completed. No module edits.

Remote build and vet:

```sh
export PATH=$HOME/.cargo/bin:/usr/local/go/bin:$PATH
cd ~/firewood
cargo build --release --locked -p firewood-ffi --features ethhash,logger --target x86_64-unknown-linux-gnu
cd ~/csbench
go vet ./bench/go/csbench
mkdir -p ~/badlib
printf 'invalid archive\n' > ~/badlib/libfirewood_ffi.a
CGO_LDFLAGS=-L/home/ubuntu/badlib go run ./bench/go/csbench -n 10 -d 2 -blocks 0 -warmup 0 -json
```

`run-cell.sh` records the full flags, affinity, thread settings, environment, memory, and file size commands. Each job is launched with `nohup bash ~/run-cell.sh ARGS > ~/NAME-launch.log 2>&1 < /dev/null &`. Exact ARGS appear in `jobs.txt`. Each step was launched and checked individually.

Residency snapshots, run during measured blocks:

```sh
ssh -i ~/.ssh/claude -o IdentitiesOnly=yes ubuntu@54.215.196.23 'python3 ~/residency.py' > bench/go/csbench/results/c8i/residency-2m.txt
ssh -i ~/.ssh/claude -o IdentitiesOnly=yes ubuntu@54.215.196.23 'tail -1 ~/results/run-bundled-n6-d38400-r1.jsonl; grep seed ~/results/run-bundled-n6-d38400-r1.stderr | tail -1; python3 ~/residency.py' > bench/go/csbench/results/c8i/residency-6m.txt
```

`residency.py` uses Linux mincore on an untouched private file mapping and prints `/proc/meminfo`. This tests residency at the snapshot, not every instant of every run.

Build identity:

```sh
sha256sum ~/csbench/go.mod ~/csbench/go.sum ~/csbench/bench/go/csbench/main.go ~/firewood/Cargo.lock ~/firewood/storage/src/hashers/{batched,lanes,lane_sponge,ethhash,mod}.rs ~/firewood/storage/src/nodestore/hash.rs ~/firewood/target/x86_64-unknown-linux-gnu/release/libfirewood_ffi.a
/usr/local/go/bin/go version
~/.cargo/bin/rustc --version
lscpu -e=CPU,CORE,SOCKET
sudo /usr/lib/linux-tools/7.0.0-1014-aws/perf stat -e cpu-clock true
```

Profiles:

```sh
nohup bash ~/profile.sh stock > ~/results/profile-stock.launch 2>&1 < /dev/null &
nohup bash ~/profile.sh lanes+par > ~/results/profile-lanes+par.launch 2>&1 < /dev/null &
/usr/lib/linux-tools/7.0.0-1014-aws/perf script -i ~/results/profile-stock.data -F comm,pid,tid | sort | uniq -c
/usr/lib/linux-tools/7.0.0-1014-aws/perf script -i ~/results/profile-lanes+par.data -F comm,pid,tid | sort | uniq -c
```

See `profile.sh` for exact process selection, sampling and symbol flags. It uses `go run -ldflags="-s=false -w=false"` because the default go run executable was verified stripped using `file` and `nm`. Initial stripped and close-inclusive profiles are retained under separate raw subdirectories and excluded from the final profile summary. Final capture begins after block 5 appears and stops after block 25 appears. Poll intervals introduce a small boundary delay. The intentional SIGINT returns 130; the final script accepts that status. The stock report was generated manually with the same perf report command after its intentional stop because the first script version treated 130 as failure.

Analysis and raw copy:

```sh
rsync -az -e 'ssh -i /Users/ilia/.ssh/claude -o IdentitiesOnly=yes' ubuntu@54.215.196.23:results/ bench/go/csbench/results/c8i/raw/
python3 bench/go/csbench/results/c8i/check-roots.py bench/go/csbench/results/c8i/raw/gate-*.jsonl
python3 bench/go/csbench/results/c8i/validate-all.py > bench/go/csbench/results/c8i/all-roots.txt
python3 bench/go/csbench/results/c8i/summarize.py > bench/go/csbench/results/c8i/matrix.md
python3 bench/go/csbench/results/c8i/gains.py > bench/go/csbench/results/c8i/gains.txt
python3 bench/go/csbench/results/c8i/repeats.py > bench/go/csbench/results/c8i/repeats.csv
python3 bench/go/csbench/results/c8i/profile-summary.py > bench/go/csbench/results/c8i/profiles.txt
```
