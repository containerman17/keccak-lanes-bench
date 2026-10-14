// Package statedb opens the Firewood adapter used by the storage workload.
package statedb

import (
	"github.com/ava-labs/avalanchego/graft/evm/firewood"
	"github.com/ava-labs/firewood-go-ethhash/ffi"
	"github.com/ava-labs/libevm/core/rawdb"
	"github.com/ava-labs/libevm/core/state"
	"github.com/ava-labs/libevm/triedb"
)

// Open uses an auxiliary memory database, with Firewood state stored on disk.
// Close drains deferred persistence after the measured blocks.
func Open(dir string, cache, revisions int) (state.Database, *triedb.Database, *ffi.Database, func()) {
	disk := rawdb.NewMemoryDatabase()
	cfg := firewood.TrieDBConfig{DatabaseDir: dir, CacheSizeBytes: uint(cache) << 20, RevisionsInMemory: uint(revisions), CacheStrategy: ffi.CacheAllReads, DeferredCommitInterval: uint64(revisions - 1)}
	tdb := triedb.NewDatabase(disk, &triedb.Config{DBOverride: cfg.BackendConstructor})
	db := firewood.NewStateAccessor(state.NewDatabaseWithNodeDB(disk, tdb))
	close := func() {
		if err := tdb.Close(); err != nil {
			panic(err)
		}
		if err := disk.Close(); err != nil {
			panic(err)
		}
	}
	return db, tdb, tdb.Backend().(*firewood.TrieDB).Firewood, close
}
