import pathlib,re
root=pathlib.Path(__file__).parent/'raw'
for mode in ['stock','lanes+par']:
 p=root/f'profile-{mode}.report'
 if not p.exists(): continue
 rows=[]
 for line in p.read_text().splitlines():
  m=re.match(r'\s*([\d.]+)%\s+\[.\]\s+(.*?)\s+-\s+-\s*$',line)
  if m: rows.append((float(m[1]),m[2]))
 print(mode,'top 25 self symbols:')
 for pct,sym in rows[:25]: print(f'{pct:6.2f}% {sym}')
 kernels=[(p,s) for p,s in rows if ('keccak' in s.lower() and ('keccak_f' in s or 'keccak_p' in s or 'keccakF' in s)) or ('lanes::x86::kernel' in s)]
 print('Keccak permutation self symbols:')
 for pct,sym in kernels: print(f'{pct:6.2f}% {sym}')
 print(f'Sum of displayed permutation self shares: {sum(p for p,s in kernels):.2f}%')
 print('Other hash-related symbols:')
 for pct,sym in rows:
  if any(w in sym.lower() for w in ['keccak','hashers::lanes','hashers::batched','hashers::ethhash']) and (pct,sym) not in kernels: print(f'{pct:6.2f}% {sym}')
 print()
