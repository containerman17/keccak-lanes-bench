# Local verification commands

Run from `firewood/csbench`, on 2026-10-14 JST. These are correctness checks,
not Mac performance results. No Mac timing files are retained.

```sh
go mod tidy
go vet ./...
go run . -n 20000 -d 500 -blocks 2 -warmup 1 -self-check
```

The self-check returned `ok:true`: seed and block state roots, storage roots,
and read checksums match between db and e2e.

The patched modes were checked with these commands (the archive path was
`/Users/ilia/.herdr/worktrees/firewood/lanes-hash/target/aarch64-apple-darwin/release`):

```sh
export CGO_LDFLAGS=-L/absolute/path/to/firewood/target/aarch64-apple-darwin/release
for mode in stock lanes par lanes+par; do
  FIREWOOD_HASH_MODE="$mode" go run . -n 20000 -d 500 -blocks 2 -warmup 1 -self-check -print-seed-root -json > "/tmp/park-$mode.jsonl"
done
python3 results/check-roots.py /tmp/park-stock.jsonl /tmp/park-lanes.jsonl /tmp/park-par.jsonl /tmp/park-lanes+par.jsonl
```

Linux build target: `x86_64-unknown-linux-gnu`. Recorded c8i benchmark commands
and their raw outputs are in `../../results/c8i-8xlarge/COMMANDS.md`.

For a rebuild after replacing the archive at the same path, add `-a` to `go run`.

All patched modes passed self-check and matched stock roots and read checksums.
This Mac uses the scalar fallback; x86 SIMD execution is not tested by this check.
