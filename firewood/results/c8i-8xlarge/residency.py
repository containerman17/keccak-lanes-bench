import ctypes,datetime,json,mmap,os,pathlib,zoneinfo
libc=ctypes.CDLL(None,use_errno=True)
libc.mincore.argtypes=[ctypes.c_void_p,ctypes.c_size_t,ctypes.c_void_p]
for p in pathlib.Path('/home/ubuntu/dbs').rglob('*'):
    if not p.is_file(): continue
    size=p.stat().st_size
    if not size: continue
    with p.open('rb') as f:
        m=mmap.mmap(f.fileno(),0,access=mmap.ACCESS_COPY)
        address=ctypes.addressof(ctypes.c_char.from_buffer(m))
        pages=(size+mmap.PAGESIZE-1)//mmap.PAGESIZE
        vec=(ctypes.c_ubyte*pages)()
        rc=libc.mincore(address,size,vec)
        if rc: raise OSError(ctypes.get_errno())
        print(json.dumps(dict(jst=datetime.datetime.now(zoneinfo.ZoneInfo('Asia/Tokyo')).isoformat(),path=str(p),file_bytes=size,pages=pages,resident_pages=sum(v&1 for v in vec))))
        m.close()
print(pathlib.Path('/proc/meminfo').read_text())
