import json
import pathlib
import statistics
import sys

for name in sys.argv[1:]:
    p = pathlib.Path(name)
    rows = [json.loads(s) for s in p.read_text().splitlines()]
    config = rows[0]
    print(f'{p.name}: n={config["n"]} d={config["d"]}')
    for level in ('db', 'e2e'):
        blocks = [r for r in rows if r['kind'] == 'block' and r['level'] == level and not r['warmup']]
        seed = next(r for r in rows if r['kind'] == 'seed' and r['level'] == level)
        print(f'  {level}: {len(blocks)} measured blocks; seed_ms={seed["total_ms"]:.3f}')
        for k in ('read_ms', 'write_ms', 'propose_ms', 'commit_ms', 'total_ms', 'us_per_dirty_slot'):
            vals = [r[k] for r in blocks]
            print(f'    {k}: mean={statistics.mean(vals):.3f} median={statistics.median(vals):.3f} min={min(vals):.3f} max={max(vals):.3f}')
        print(f'    seed_storage_root={seed["storage_root"]}')
        print(f'    final_storage_root={blocks[-1]["storage_root"]}')
        print(f'    final_state_root={blocks[-1]["root"]}')
    assert rows[-1]['kind'] == 'self_check' and rows[-1]['ok']
    print(f'  self_check={rows[-1]}')
