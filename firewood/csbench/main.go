// csbench measures one contract with random storage updates through Firewood cgo.
package main

import (
	"context"
	"encoding/binary"
	"encoding/json"
	"flag"
	"fmt"
	"math/rand"
	"os"
	"path/filepath"
	"time"

	"github.com/ava-labs/firewood-go-ethhash/ffi"
	"github.com/ava-labs/libevm/common"
	"github.com/ava-labs/libevm/core/state"
	"github.com/ava-labs/libevm/core/types"
	"github.com/ava-labs/libevm/crypto"
	"github.com/ava-labs/libevm/libevm/stateconf"
	"github.com/ava-labs/libevm/rlp"
	"github.com/ava-labs/libevm/triedb"
	"github.com/containerman17/keccak-lanes-bench/firewood/csbench/internal/statedb"
)

var addr = common.HexToAddress("0x0000000000000000000000000000000000000100")
var accountKey = crypto.Keccak256(addr[:])
var jsonOut bool

type result struct {
	Kind     string      `json:"kind"`
	Level    string      `json:"level"`
	Block    int         `json:"block"`
	Warmup   bool        `json:"warmup"`
	Dirty    int         `json:"dirty"`
	Read     float64     `json:"read_ms"`
	Write    float64     `json:"write_ms"`
	Propose  float64     `json:"propose_ms"`
	Commit   float64     `json:"commit_ms"`
	Total    float64     `json:"total_ms"`
	US       float64     `json:"us_per_dirty_slot"`
	Root     common.Hash `json:"root"`
	Storage  common.Hash `json:"storage_root"`
	Checksum common.Hash `json:"read_xor"`
}

func emit(v any) {
	if jsonOut {
		must(json.NewEncoder(os.Stdout).Encode(v))
	} else {
		fmt.Printf("%+v\n", v)
	}
}
func must(err error) {
	if err != nil {
		panic(err)
	}
}
func word(v uint64) (h common.Hash) { binary.BigEndian.PutUint64(h[24:], v); return }
func value(slot uint64, generation uint64) common.Hash {
	h := word(slot + 1)
	binary.BigEndian.PutUint64(h[16:24], generation+1)
	return h
}
func key(k common.Hash) []byte {
	return append(append([]byte{}, accountKey...), crypto.Keccak256(k[:])...)
}
func encoded(v common.Hash) []byte {
	b, e := rlp.EncodeToBytes(common.TrimLeftZeroes(v[:]))
	must(e)
	return b
}
func ms(d time.Duration) float64 { return float64(d) / float64(time.Millisecond) }

type engine struct {
	level  string
	fw     *ffi.Database
	tdb    *triedb.Database
	db     state.Database
	root   common.Hash
	height uint64
	close  func()
}

func open(level, dir string, cache, revisions int) *engine {
	e := &engine{level: level, root: types.EmptyRootHash}
	if level == "db" {
		var err error
		e.fw, err = ffi.New(filepath.Join(dir, "firewood"), ffi.EthereumNodeHashing, ffi.WithNodeCacheSizeInBytes(uint(cache)<<20), ffi.WithRevisions(uint(revisions)), ffi.WithReadCacheStrategy(ffi.CacheAllReads), ffi.WithDeferredPersistenceCommitCount(uint64(revisions-1)))
		must(err)
		e.close = func() { must(e.fw.Close(context.Background())) }
	} else {
		e.db, e.tdb, e.fw, e.close = statedb.Open(dir, cache, revisions)
	}
	return e
}
func (e *engine) storageRoot() common.Hash {
	r, err := e.fw.LatestRevision()
	must(err)
	defer r.Drop()
	p, err := r.EthGetProof(accountKey, nil)
	must(err)
	return common.Hash(p.StorageHash)
}
func (e *engine) apply(slots []uint64, gen uint64, read bool) result {
	out := result{Level: e.level, Dirty: len(slots)}
	start := time.Now()
	var s *state.StateDB
	var ops []ffi.BatchOp
	if e.level == "e2e" {
		var err error
		s, err = state.New(e.root, e.db, nil)
		must(err)
	}
	if read {
		for _, id := range slots {
			k := word(id)
			var v common.Hash
			if s != nil {
				v = s.GetState(addr, k)
			} else {
				b, err := e.fw.Get(key(k))
				must(err)
				var raw []byte
				must(rlp.DecodeBytes(b, &raw))
				v = common.BytesToHash(raw)
			}
			for j := range v {
				out.Checksum[j] ^= v[j]
			}
		}
	}
	t1 := time.Now()
	out.Read = ms(t1.Sub(start))
	if s != nil {
		if e.height == 0 {
			s.CreateAccount(addr)
			s.SetNonce(addr, 1)
		}
		for _, id := range slots {
			s.SetState(addr, word(id), value(id, gen))
		}
		must(s.Error())
	} else {
		ops = make([]ffi.BatchOp, 0, len(slots)+1)
		if e.height == 0 {
			a := types.NewEmptyStateAccount()
			a.Nonce = 1
			b, err := rlp.EncodeToBytes(a)
			must(err)
			ops = append(ops, ffi.Put(accountKey, b))
		}
		for _, id := range slots {
			ops = append(ops, ffi.Put(key(word(id)), encoded(value(id, gen))))
		}
	}
	t2 := time.Now()
	out.Write = ms(t2.Sub(t1))
	var p *ffi.Proposal
	if s != nil {
		s.Finalise(true)
		e.root = s.IntermediateRoot(true)
		must(s.Error())
	} else {
		var err error
		p, err = e.fw.Propose(ops)
		must(err)
		e.root = common.Hash(p.Root())
	}
	t3 := time.Now()
	out.Propose = ms(t3.Sub(t2))
	if s != nil {
		root, err := s.Commit(e.height, true, stateconf.WithTrieDBUpdateOpts(stateconf.WithTrieDBUpdatePayload(word(e.height), word(e.height+1))))
		must(err)
		if root != e.root {
			panic("IntermediateRoot and Commit disagree")
		}
		must(e.tdb.Commit(root, false))
	} else {
		must(p.Commit())
	}
	out.Commit = ms(time.Since(t3))
	out.Total = ms(time.Since(start))
	out.US = out.Total * 1000 / float64(len(slots))
	out.Root = e.root
	e.height++
	// Proof extraction is outside the measured block.
	out.Storage = e.storageRoot()
	return out
}
func run(level, parent string, n, d, blocks, warmup, cache, revisions int, seed int64, printSeed bool) []result {
	dir, err := os.MkdirTemp(parent, "csbench-"+level+"-")
	must(err)
	e := open(level, dir, cache, revisions)
	defer func() { e.close(); must(os.RemoveAll(dir)) }()
	start := time.Now()
	for lo := 0; lo < n; lo += 100000 {
		slots := make([]uint64, min(100000, n-lo))
		for i := range slots {
			slots[i] = uint64(lo + i)
		}
		e.apply(slots, 0, false)
		fmt.Fprintf(os.Stderr, "%s %s seed %d/%d elapsed %.1fs\n", time.Now().In(time.FixedZone("JST", 9*3600)).Format("2006-01-02 15:04:05 JST"), level, lo+len(slots), n, time.Since(start).Seconds())
	}
	seedResult := result{Kind: "seed", Level: level, Root: e.root, Storage: e.storageRoot(), Total: ms(time.Since(start))}
	if printSeed {
		emit(seedResult)
	}
	results := []result{seedResult}
	rng := rand.New(rand.NewSource(seed))
	for b := 0; b < warmup+blocks; b++ {
		// Sample without replacement so D means exactly D dirty slots. Generate
		// the workload outside the clock, independently of backend iteration order.
		slots := make([]uint64, 0, d)
		seen := make(map[uint64]struct{}, d)
		for len(slots) < d {
			id := uint64(rng.Int63n(int64(n)))
			if _, ok := seen[id]; !ok {
				seen[id] = struct{}{}
				slots = append(slots, id)
			}
		}
		r := e.apply(slots, uint64(b+1), true)
		r.Kind = "block"
		r.Block = b + 1
		r.Warmup = b < warmup
		emit(r)
		results = append(results, r)
	}
	return results
}
func main() {
	level := flag.String("level", "db", "db or e2e")
	n := flag.Int("n", 2000000, "seeded storage slots")
	d := flag.Int("d", 38400, "distinct dirty slots per block")
	blocks := flag.Int("blocks", 20, "measured blocks")
	warmup := flag.Int("warmup", 5, "warmup blocks")
	cache := flag.Int("cache-mb", 256, "Firewood node cache MiB")
	revisions := flag.Int("revisions", 32, "retained revisions; persistence deferred by revisions minus one")
	dir := flag.String("datadir", os.TempDir(), "parent directory for a fresh temporary DB, removed on success")
	seed := flag.Int64("seed", 1, "random slot sequence seed")
	printSeed := flag.Bool("print-seed-root", false, "print seed state and storage roots")
	check := flag.Bool("self-check", false, "run both levels sequentially and compare all roots and read checksums")
	flag.BoolVar(&jsonOut, "json", false, "JSON lines on stdout; progress on stderr")
	flag.Parse()
	if (*level != "db" && *level != "e2e") || *n < 1 || *d < 1 || *d > *n || *blocks < 0 || *warmup < 0 || *cache < 1 || *revisions < 2 {
		panic("invalid flags")
	}
	emit(map[string]any{"kind": "config", "level": *level, "n": *n, "d": *d, "blocks": *blocks, "warmup": *warmup, "cache_mb": *cache, "revisions": *revisions, "seed": *seed, "account": addr, "self_check": *check})
	if *check {
		a := run("db", *dir, *n, *d, *blocks, *warmup, *cache, *revisions, *seed, *printSeed)
		b := run("e2e", *dir, *n, *d, *blocks, *warmup, *cache, *revisions, *seed, *printSeed)
		for i := range a {
			if a[i].Root != b[i].Root || a[i].Storage != b[i].Storage || a[i].Checksum != b[i].Checksum {
				panic(fmt.Sprintf("self-check mismatch at index %d: %+v vs %+v", i, a[i], b[i]))
			}
		}
		emit(map[string]any{"kind": "self_check", "ok": true, "compared": len(a)})
	} else {
		run(*level, *dir, *n, *d, *blocks, *warmup, *cache, *revisions, *seed, *printSeed)
	}
}
