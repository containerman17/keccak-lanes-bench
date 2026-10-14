import collections,json,pathlib
root=pathlib.Path(__file__).parent/'raw'
base={}; count=collections.Counter()
for p in sorted(root.glob('*.jsonl')):
    if not (p.stem.startswith('gate-') or p.stem.startswith('run-')): continue
    if not p.with_suffix('.exit').exists(): continue
    assert p.with_suffix('.exit').read_text().strip()=='0',p
    rows=[json.loads(x) for x in p.read_text().splitlines()]
    cfg=rows[0]; key=(cfg['n'],cfg['d'],cfg['seed'],cfg['blocks'],cfg['warmup'])
    for level in ['db','e2e']:
        values=[tuple(r[f] for f in ['kind','block','root','storage_root','read_xor']) for r in rows if r['kind'] in ['seed','block'] and r['level']==level]
        if not values: continue
        assert len(values)==1+cfg['blocks']+cfg['warmup'],p
        if key not in base: base[key]=values
        assert values==base[key],(p,level,'mismatch')
        count[key]+=1
for key,n in sorted(count.items()): print(key, n, 'level-runs match every seed/block state root, storage root, and read checksum')
