#!/bin/bash
set -euo pipefail
export PATH=/usr/local/go/bin:$PATH
mode=$1
out=profile-$mode${SUFFIX:-}
export FIREWOOD_HASH_MODE=$mode CGO_LDFLAGS=-L/home/ubuntu/firewood/target/x86_64-unknown-linux-gnu/release GOMAXPROCS=16 RAYON_NUM_THREADS=16
cd ~/csbench
set -x
taskset -c 0-15 go run -ldflags="-s=false -w=false" ./bench/go/csbench -level db -n 2000000 -d 38400 -cache-mb 1024 -revisions 32 -seed 1 -blocks 20 -warmup 5 -datadir /home/ubuntu/dbs -print-seed-root -json > ~/results/$out.jsonl 2> ~/results/$out.stderr &
job=$!
until grep -q '"block":5,' ~/results/$out.jsonl; do
  kill -0 "$job"
  sleep 0.1
done
pid=$(pgrep -n -x csbench)
sudo /usr/lib/linux-tools/7.0.0-1014-aws/perf record -e cpu-clock -F ${FREQ:-499} ${CG:--g} -p "$pid" -o /home/ubuntu/results/$out.data &
prof=$!
until grep -q '"block":25,' ~/results/$out.jsonl; do
  kill -0 "$job"
  sleep 0.05
done
sudo kill -INT "$prof"
wait "$prof" || test "$?" -eq 130
wait "$job"
sudo /usr/lib/linux-tools/7.0.0-1014-aws/perf report -i /home/ubuntu/results/$out.data --stdio --no-children --sort symbol --call-graph none --percent-limit 0 > ~/results/$out.report
sudo /usr/lib/linux-tools/7.0.0-1014-aws/perf report -i /home/ubuntu/results/$out.data --stdio --no-children --percent-limit 0.8 > ~/results/$out.callgraph
sudo chown ubuntu:ubuntu ~/results/$out.data
echo 0 > ~/results/$out.exit
