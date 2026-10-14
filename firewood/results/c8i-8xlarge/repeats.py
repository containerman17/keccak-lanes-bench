import json,pathlib,statistics
root=pathlib.Path(__file__).parent/'raw'
print('file,level,median_total_ms,median_propose_ms,min_total_ms,max_total_ms')
for p in sorted(root.glob('*.jsonl')):
 if not (p.stem.startswith('gate-') or p.stem.startswith('run-')): continue
 if not p.with_suffix('.exit').exists() or p.with_suffix('.exit').read_text().strip()!='0': continue
 rows=[json.loads(x) for x in p.read_text().splitlines()]
 for level in ['db','e2e']:
  b=[r for r in rows if r['kind']=='block' and r['level']==level and not r['warmup']]
  if not b: continue
  print(f"{p.name},{level},{statistics.median(r['total_ms'] for r in b):.6f},{statistics.median(r['propose_ms'] for r in b):.6f},{min(r['total_ms'] for r in b):.6f},{max(r['total_ms'] for r in b):.6f}")
