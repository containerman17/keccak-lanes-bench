# Experimental proposal hashing

This branch starts at Firewood v0.8.0 and keeps its FFI API. Build once with
`ethhash`; select a mode before starting each process:

- `FIREWOOD_HASH_MODE=stock`: depth-first scalar hashing, the default.
- `FIREWOOD_HASH_MODE=lanes`: collect dirty storage nodes by level and batch
  independent Keccak messages. x86 selects AVX-512 or AVX2 at runtime; other
  targets and lone messages use sha3.
- `FIREWOOD_HASH_MODE=par`: split dirty branch children into Rayon tasks with the
  stock scalar encoder.
- `FIREWOOD_HASH_MODE=lanes+par`: batch levels inside the independent branch tasks.

The first lookup caches the mode with `OnceLock`. Missing or unknown values
select stock. Restart the process to change modes. Without the `ethhash`
feature, the mode has no effect.

## Implementation boundaries

Account nodes and the single-storage-child fake root use the stock path.
The optimized walk starts below the account and visits only dirty `Child::Node`
entries. Levels are collected separately for each account storage child, not
across all storage children together. Persisted child hashes remain available
to their parents.

The walk detaches nodes into levels, then rebuilds from the deepest level.
Compressed branches first batch the internal branch digest, then encode and
batch the outer extension digest. The encoder is shared with stock mode.
Inline RLP retains the stock length rule.

Parallel scheduling uses a bounded dirty-node scan before creating a Rayon
scope for sibling branches. Tasks do not create further tasks. Scalar tasks
use an iterative depth-first walk with a reused path; combined-mode tasks use
level batching. The constants in `storage/src/hashers/batched.rs` keep small
subtrees serial. Calls from `ParallelMerkle` reuse its current Rayon pool;
calls outside that pool use Rayon's global pool. This avoids a recursive task
per trie level. Combined mode falls back to serial lane batching when its
initial branch does not meet the split cutoff.

The copied Keccak permutation comes from the local keccak-lanes-bench project.
The adapter groups variable-length messages by full-block count and uses
per-message padding. Pointer reads use unaligned loads from each original
pointer instead of gather offsets across allocations. Original kernel benchmark
results therefore do not establish performance of this adapter.

## Build the FFI library

The distribution alias in `.cargo/config.toml` uses maxperf and enables
`ethhash,logger`. The requested release builds use the same feature set:

```sh
cargo build --release --locked -p firewood-ffi --features ethhash,logger --target aarch64-apple-darwin
cargo build --release --locked -p firewood-ffi --features ethhash,logger --target x86_64-unknown-linux-gnu
```

Outputs are `target/<target>/release/libfirewood_ffi.a`. Run the Linux command
on the coordinator's Linux host. Linux linking and SIMD execution are
UNVERIFIED by the Mac build. The Go harness must select this library when it
links, then use separate processes for mode comparisons.

## Verify and measure

Run the following test command separately with each mode:

```sh
FIREWOOD_HASH_MODE=stock cargo test -p firewood-storage -p firewood --features ethhash
FIREWOOD_HASH_MODE=lanes cargo test -p firewood-storage -p firewood --features ethhash
FIREWOOD_HASH_MODE=par cargo test -p firewood-storage -p firewood --features ethhash
FIREWOOD_HASH_MODE=lanes+par cargo test -p firewood-storage -p firewood --features ethhash
```

The integration test starts a separate process for each mode and compares
account roots across insertion, overwrite, deletion, and single-child
transitions. Storage tests cover inline RLP, extension digests, parallel levels,
small-stack traversal, and message block boundaries. On x86, the kernel test
runs each available vector implementation directly.

## Files in the patch

- `storage/src/hashers/batched.rs`, `lanes.rs`, and `lane_sponge.rs`: scheduling, permutation, and variable-length sponge.
- `storage/src/hashers/ethhash.rs`, `mod.rs`, and `storage/src/nodestore/hash.rs`: shared encoding and mode dispatch.
- `storage/Cargo.toml` and `Cargo.lock`: dependencies.
- `firewood/examples/hash_account.rs` and `firewood/tests/hash_account_modes.rs`: reproducible account workload and cross-mode root comparison.
- `hashing-experiment.md`: original implementation notes.

## Apply

From a clean Firewood v0.8.0 checkout:

```sh
git apply --check /absolute/path/to/keccak-lanes-bench/firewood/lanes-hash-v0.8.0.patch
git apply /absolute/path/to/keccak-lanes-bench/firewood/lanes-hash-v0.8.0.patch
```

The patch was regenerated with `git diff --cached HEAD`. All source additions
were already staged. The only untracked directory was `evidence/`, which holds
local Mac logs and a redundant patch, and is excluded. No Firewood commit was made.
