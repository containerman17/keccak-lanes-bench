#!/bin/bash
set -euo pipefail
export PATH=/usr/local/go/bin:$HOME/.cargo/bin:$PATH
name=$1; mode=$2; level=$3; n=$4; d=$5; cache=$6; blocks=${7:-20}; warmup=${8:-5}; check=${9:-}
cd ~/csbench
export GOMAXPROCS=16 RAYON_NUM_THREADS=16 FIREWOOD_HASH_MODE=$mode
if [ "$mode" != bundled ]; then export CGO_LDFLAGS=-L/home/ubuntu/firewood/target/x86_64-unknown-linux-gnu/release; else unset CGO_LDFLAGS; fi
exec > ~/results/$name.stdout 2> ~/results/$name.stderr
set -x
TZ=Asia/Tokyo date
free -b
df -B1 /
set +e
taskset -c 0-15 go run ./bench/go/csbench -level "$level" -n "$n" -d "$d" -cache-mb "$cache" -revisions 32 -seed 1 -blocks "$blocks" -warmup "$warmup" -datadir /home/ubuntu/dbs -print-seed-root -json $check > ~/results/$name.jsonl &
job=$!
while kill -0 "$job" 2>/dev/null; do
  TZ=Asia/Tokyo date
  free -b
  du -sb ~/dbs
  find ~/dbs -type f -printf '%s %p\n'
  sleep 5
done > ~/results/$name.memory
wait "$job"
rc=$?
echo "$rc" > ~/results/$name.exit
exit "$rc"
