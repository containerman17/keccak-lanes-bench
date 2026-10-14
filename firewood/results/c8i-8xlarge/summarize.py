import collections,json,pathlib,statistics
root=pathlib.Path(__file__).parent
fields=['read_ms','write_ms','propose_ms','commit_ms','total_ms','us_per_dirty_slot']
groups=collections.defaultdict(list)
for p in sorted((root/'raw').glob('*.jsonl')):
    if not (p.with_suffix('.exit').exists() and p.with_suffix('.exit').read_text().strip()=='0'): continue
    if not (p.stem.startswith('gate-') or p.stem.startswith('run-')): continue
    rows=[json.loads(s) for s in p.read_text().splitlines()]
    cfg=rows[0]
    mode=p.stem.split('-')[1]
    for level in ['db','e2e']:
        blocks=[r for r in rows if r['kind']=='block' and r['level']==level and not r['warmup']]
        if not blocks: continue
        assert len(blocks)==20,(p,len(blocks))
        key=(cfg['n'],cfg['d'],cfg['cache_mb'],level,mode)
        med={f:statistics.median(r[f] for r in blocks) for f in fields}
        groups[key].append((p.name,med))
print('| N | D | cache MiB | level | mode | repeats | read ms | write ms | propose ms | commit ms | total ms | us per dirty slot |')
print('|---:|---:|---:|:---|:---|---:|---:|---:|---:|---:|---:|---:|')
summary=[]
for key,runs in sorted(groups.items()):
    means={f:statistics.mean(r[f] for _,r in runs) for f in fields}
    print('| '+' | '.join(map(str,key))+f' | {len(runs)} | '+' | '.join(f'{means[f]:.3f}' for f in fields)+' |')
    summary.append(dict(zip(['n','d','cache','level','mode'],key),repeats=len(runs),files=[x for x,_ in runs],**means))
(root/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
