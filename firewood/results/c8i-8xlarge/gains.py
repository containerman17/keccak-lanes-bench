# Percentages vs modified stock, from summary.json written by summarize.py.
# speedup = stock_ms / mode_ms; reduction = 1 - mode_ms / stock_ms.
import json,pathlib
rows=json.loads((pathlib.Path(__file__).parent/'summary.json').read_text())
keys=sorted({(r['n'],r['d'],r['cache'],r['level']) for r in rows})
print('| N | D | cache MiB | level | mode | propose speedup | propose reduction | total speedup | total reduction |')
print('|---:|---:|---:|:---|:---|---:|---:|---:|---:|')
for k in keys:
    r={x['mode']:x for x in rows if (x['n'],x['d'],x['cache'],x['level'])==k}
    s=r.get('stock')
    if not s: continue
    for m in ['bundled','lanes','par','lanes+par']:
        if m not in r: continue
        a=r[m]
        print(f"| {k[0]} | {k[1]} | {k[2]} | {k[3]} | {m} | {s['propose_ms']/a['propose_ms']:.3f}x | {100*(1-a['propose_ms']/s['propose_ms']):.1f}% | {s['total_ms']/a['total_ms']:.3f}x | {100*(1-a['total_ms']/s['total_ms']):.1f}% |")
print()
print('| N | D | cache MiB | level | stock us/dirty slot | lanes+par us/dirty slot | bundled us/dirty slot |')
print('|---:|---:|---:|:---|---:|---:|---:|')
for k in keys:
    r={x['mode']:x for x in rows if (x['n'],x['d'],x['cache'],x['level'])==k}
    g=lambda m: f"{r[m]['us_per_dirty_slot']:.2f}" if m in r else 'n/a'
    print(f"| {k[0]} | {k[1]} | {k[2]} | {k[3]} | {g('stock')} | {g('lanes+par')} | {g('bundled')} |")
