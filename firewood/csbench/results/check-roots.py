"""Compare seed/block roots and read checksums across matching workloads."""
import json
import sys


def roots(path):
    rows = [json.loads(line) for line in open(path)]
    return [(r['kind'], r['level'], r['block'], r['root'], r['storage_root'], r['read_xor'])
            for r in rows if r['kind'] in ('seed', 'block')]


if len(sys.argv) < 3:
    sys.exit('usage: python3 check-roots.py baseline.jsonl candidate.jsonl [...]')
expected = roots(sys.argv[1])
assert expected, 'baseline has no roots'
for path in sys.argv[2:]:
    assert roots(path) == expected, f'root or checksum mismatch: {path}'
    print(f'{path}: roots and read checksums match')
