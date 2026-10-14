# Packaging verification

Run on 2026-10-14 JST. No Mac performance results are included.

The standalone module passed the commands in
[csbench/results/COMMANDS.md](../csbench/results/COMMANDS.md), including all
patched modes. The local linker trace selected the patched archive:

```sh
cd firewood/csbench
CGO_LDFLAGS='-L/Users/ilia/.herdr/worktrees/firewood/lanes-hash/target/aarch64-apple-darwin/release -Wl,-t' FIREWOOD_HASH_MODE=lanes+par go run . -n 20 -d 5 -blocks 1 -warmup 0 -self-check > /tmp/park-link.stdout 2> /tmp/park-link.stderr
rg -m 2 'lanes-hash/.*/libfirewood_ffi.a' /tmp/park-link.stdout /tmp/park-link.stderr
```

The adapter extraction also preserves the source harness's roots and read checksums:

```sh
cd /Users/ilia/temp-1m-clob
go run ./bench/go/csbench -n 20000 -d 500 -blocks 2 -warmup 1 -self-check -print-seed-root -json > /tmp/park-original.jsonl 2> /tmp/park-original.stderr
python3 /Users/ilia/.herdr/worktrees/keccak-lanes-bench/main/firewood/csbench/results/check-roots.py /tmp/park-original.jsonl /tmp/park-stock.jsonl
```

The c8i runs used the original harness before standalone packaging. The timed
block code is unchanged; only adapter setup moved into `internal/statedb`.
Standalone c8i timing after packaging is UNVERIFIED.

The patch applies to the release in an isolated Git index, without changing
any Firewood worktree or its staged changes:

```python
import os, subprocess, tempfile
from pathlib import Path
source = '/Users/ilia/.herdr/worktrees/firewood/lanes-hash'
with tempfile.TemporaryDirectory(prefix='park-patch-') as d:
    env = dict(os.environ, GIT_INDEX_FILE=d + '/index')
    subprocess.run(['git', '-C', source, 'read-tree', 'v0.8.0^{commit}'], env=env, check=True)
    subprocess.run(['git', '-C', source, 'apply', '--cached', '--check',
                    str(Path('firewood/lanes-hash-v0.8.0.patch').resolve())], env=env, check=True)
```

The patch preserves original removed lines and context exactly. Blank context
lines and punctuation in removed upstream comments are not source additions.

Final published results were verified from the repository root:

```sh
python3 firewood/results/c8i-8xlarge/summarize.py > /tmp/park-matrix.md
cmp firewood/results/c8i-8xlarge/matrix.md /tmp/park-matrix.md
python3 firewood/results/c8i-8xlarge/gains.py > /tmp/park-gains.txt
cmp firewood/results/c8i-8xlarge/gains.txt /tmp/park-gains.txt
python3 firewood/results/c8i-8xlarge/validate-all.py
```

Both comparisons passed, and all workload root comparisons passed.
