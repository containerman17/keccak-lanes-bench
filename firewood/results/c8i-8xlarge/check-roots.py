import json,sys
baseline=None
for path in sys.argv[1:]:
    rows=[json.loads(x) for x in open(path)]
    roots=[(r['kind'],r['level'],r['block'],r['root'],r['storage_root'],r['read_xor']) for r in rows if r['kind'] in ('seed','block')]
    assert any(r.get('kind')=='self_check' and r.get('ok') for r in rows),path
    if baseline is None: baseline=roots
    assert roots==baseline, 'root mismatch: '+path
    print(path, len(roots), 'roots and checksums match')
