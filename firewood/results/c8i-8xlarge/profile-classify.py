# Usage: python3 profile-classify.py MODE REPORT SCRIPT_TXT
# REPORT: perf report --sort symbol --call-graph none (self time) of the -g -F 499 run.
# SCRIPT_TXT: perf script --no-inline -F comm,tid,ip,sym,dso --tid T of the dwarf run, one call per thread, concatenated, dso suffix stripped
# (a single unfiltered perf script call returned about 4000 samples with no frames on this perf build).
import collections,re,sys
mode,report,script=sys.argv[1:4]
self=collections.Counter()
for line in open(report):
    m=re.match(r'\s*([\d.]+)%\s+\[(.)\]\s+(.*?)\s*(-\s+-\s*)?$',line)
    if m: self[m[3]]+=float(m[1])
print(f'== {mode}: top 30 self-time symbols (perf record -e cpu-clock -F 499 -g, measured blocks 6-25, same-name entries merged) ==')
for sym,p in self.most_common(30): print(f'{p:6.2f}%  {sym[:200]}')
def kec(s): return bool(re.search(r'keccak|sha3|Keccak|lanes::x86|lane_sponge',s))
gok=sum(p for s,p in self.items() if kec(s) and ('golang.org' in s or 'libevm' in s))
rk=sum(p for s,p in self.items() if kec(s))-gok
print(f'\nself-time keccak symbols: Rust/Firewood {rk:.2f}%, Go (csbench key/slot hashing) {gok:.2f}%')
# call-chain attribution from the dwarf run, first matching rule wins
rules=[
 ('keccak (Firewood, Rust)', lambda f: kec(f[0]) and not ('golang.org' in f[0] or 'libevm' in f[0])),
 ('keccak (Go side)', lambda f: kec(f[0])),
 ('hash traversal (non-keccak)', lambda f: any(re.search(r'hash_helper|hashers::|nodestore::hash|children_hashes|hashednode|hashedshunt|HashableShunt|hash_subtrie|hash_level|hash_depth_first|ethhash',x) for x in f)),
 ('pread/pwrite + page cache', lambda f: any(re.search(r'pread|pwrite|vfs_read|vfs_write|filemap_|copy_page_to_iter|copy_page_from_iter',x) for x in f)),
 ('background persist worker (non-I/O)', lambda f: any('PersistLoop' in x or 'persist_worker' in x for x in f)),
 ('read_for_update/insert (propose)', lambda f: any(re.search(r'read_for_update|insert_helper|::insert<|ParallelMerkle|remove_helper|Merkle<.*::insert',x) for x in f) and not any('lru_mem' in x for x in f[:1])),
 ('node clone, caller truncated by 8 KiB dwarf stack', lambda f: 'BranchNode> as core::clone::Clone' in f[-1] or 'Child as core::clone::Clone' in f[-1]),
 ('get/read path', lambda f: any(re.search(r'get_helper|read_cached_node|read_node|::get<',x) for x in f)),
 ('Go runtime', lambda f: f[0].startswith('runtime.') or f[0].startswith('runtime/')),
 ('other Go', lambda f: '::' not in f[0] and '.' in f[0] and not f[0].startswith('[')),
 ('kernel other', lambda f: any(re.search(r'entry_SYSCALL|asm_exc|asm_sysvec|futex|schedule',x) for x in f)),
 ('other/unattributed', lambda f: True),
]
samples=[];cur=None
for line in open(script):
    if not line.strip(): continue
    if not line[0].isspace(): cur=[]; samples.append(cur); continue
    p=line.strip().split(' ',1); cur.append(p[1] if len(p)>1 else '?')
samples=[s for s in samples if s]
c=collections.Counter()
for f in samples:
    for name,fn in rules:
        if fn(f): c[name]+=1; break
print(f'\ncall-chain attribution (perf record -e cpu-clock -F 499 --call-graph dwarf,8192, {len(samples)} samples, first matching rule wins):')
for name,_ in rules: print(f'{100*c[name]/len(samples):6.2f}%  {name}')
