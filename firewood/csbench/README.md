# CS synthetic Firewood benchmark

Run from `firewood/csbench`. This is a standalone Go module.

```sh
go run . -level db -n 2000000 -d 38400 -blocks 20 -warmup 5 -cache-mb 256 -revisions 32 -seed 1 -print-seed-root -json
go run . -level e2e -n 2000000 -d 38400 -blocks 20 -warmup 5 -cache-mb 256 -revisions 32 -seed 1 -print-seed-root -json
go run . -n 1000 -d 100 -blocks 2 -warmup 1 -self-check -print-seed-root -json
go vet ./...
```

`-self-check` runs db then e2e, checks the seed and every block, and fails on any state-root, storage-root, or read-checksum mismatch. It uses the requested sizes, not a hidden reduced workload. JSON is line-delimited on stdout. Seed progress is on stderr in JST. Warmup blocks are emitted with `warmup:true`; exclude them when aggregating.

`-datadir` names an existing parent directory. Each run creates a fresh child, closes it and removes it on success. It does not reopen a previous seed. No code executes on a chain.

## Workload

`internal/statedb` contains the Firewood adapter setup, trimmed to this workload.
It uses an auxiliary in-memory rawdb because there is no bytecode or snapshot data.
Firewood itself uses its on-disk database.

The account is `0x0000000000000000000000000000000000000100`, nonce one, balance zero, empty code. A stateful precompile does not need stored EVM bytecode. Slot IDs are big-endian words from zero through N minus one. Values contain generation plus one followed by slot ID plus one, each as a big-endian uint64, in the low half of a word. Every write changes a nonzero value, with no deletions. Seeding commits batches of up to 100000 slots, generation zero.

The direct path uses the same encoding as the adapter's `firewood/base_trie.go`: account key = Keccak(address), storage key = Keccak(address) concatenated with Keccak(slot), value = RLP(trimmed word), account value = RLP(StateAccount). Firewood builds the Ethereum account and storage tries. Its root is the state root, not the storage root.

Each block chooses exactly D distinct random slots without replacement, using Go math/rand NewSource(seed). It reads all D slots, then writes them, then proposes and commits. Workload generation is outside the measured interval. Both levels receive identical sequences. This models storage work only: no transaction executor, gas, interpreter, row relationships, or per-transaction Finalise. The default block count and warmup count follow the brief. The seed flag controls the random block sequence; initial slot values are fixed across seed flags.

The adapter's storageTrie.Hash returns zero because Firewood manages storage roots internally. This program gets the real storage root from `LatestRevision().EthGetProof(accountKey,nil).StorageHash` after each commit. The proof and JSON emission are outside the measured interval, but can warm the account path for the next block.

## Timings

All durations are wall time. No CPU affinity is applied. One Go caller executes the storage loop; Firewood can use background persistence threads.

- `read_ms`: fresh StateDB creation for e2e and all reads, including key hashing and RLP decoding in db. Read values feed an XOR checksum.
- `write_ms`: batch allocation, key hashing, value encoding for db, or SetState calls for e2e.
- `propose_ms`: FFI Propose for db; Finalise plus IntermediateRoot for e2e. The latter invokes the Firewood proposal and includes adapter overhead.
- `commit_ms`: Proposal.Commit for db; StateDB.Commit with block-hash payload plus TrieDB.Commit for e2e.
- `total_ms`: all four stages, including their timer overhead. `us_per_dirty_slot` uses this total, not just propose time.

Both use CacheAllReads, pruning (no root store), and deferred persistence equal to revisions minus one. Commit is not a per-block fsync measurement. Database close can flush deferred work outside the block timers. With the default seed size, some seed revisions still exist during early measured blocks. Keep all settings identical in A/B runs.

## Override the Rust static library

Verified on this Mac with ffi v0.8.0. Its cgo directives use `-L${SRCDIR}/libs/aarch64-apple-darwin -lfirewood_ffi`. The environment's search directory takes precedence:

```sh
CGO_LDFLAGS='-L/absolute/path/to/modified/library' go run -a . -level db -n 2000000 -d 38400 -print-seed-root -json
```

The directory must contain `libfirewood_ffi.a` with the expected FFI ABI and Ethereum hashing support. `-a` forces rebuilding so replacing a library at the same path cannot reuse a stale Go link. It is not required for ordinary stock runs. A path without spaces is recommended.

Local verification commands are in `results/COMMANDS.md`. The c8i commands,
raw runs, and profiles are in `../results/c8i-8xlarge/`. Mac performance files
are excluded. For link diagnostics on Mac, add `-Wl,-t`; omit it for timings.
`otool -L` and `ldd` cannot identify the origin of a static archive.
Use the patched library from [the build instructions](../HASHING.md) and start
a separate process with each `FIREWOOD_HASH_MODE` value. A bundled stock run
does not test the experimental modes.

For a later stock-versus-modified comparison, save both JSON outputs with identical flags and compare roots and read checksums, excluding timings:

```sh
python3 - stock.jsonl modified.jsonl <<'PY'
import json, sys

def roots(path):
    rows = [json.loads(line) for line in open(path)]
    return [(r['kind'], r['level'], r['block'], r['root'], r['storage_root'], r['read_xor'])
            for r in rows if r['kind'] in ('seed', 'block')]

assert roots(sys.argv[1]) == roots(sys.argv[2]), 'root or checksum mismatch'
print('roots and read checksums match')
PY
```
