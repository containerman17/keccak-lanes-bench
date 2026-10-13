//! Multi-lane Keccak-256 sponge around any W-way Keccak-f[1600] kernel.

pub const RATE: usize = 136;

/// `st[i][k]` is state lane `i` of message `k`, which is the SIMD register layout.
pub type State<const W: usize> = [[u64; W]; 25];

/// A kernel XORs one 136-byte block from each of the W pointers into the state, then runs
/// Keccak-f[1600]. Every pointer must be valid for 136 bytes.
pub type Kernel<const W: usize> = unsafe fn(&mut State<W>, &[*const u8; W]);

#[repr(align(64))]
struct Aligned<T>(T);

/// Hashes `msgs` (all the same length) in groups of W. A group with fewer than W messages
/// still runs the full W-lane kernel; the idle lanes hash zeros and are discarded.
///
/// # Safety
/// `kernel` must be safe to call on this CPU.
pub unsafe fn hash<const W: usize>(msgs: &[&[u8]], out: &mut [[u8; 32]], kernel: Kernel<W>) {
    assert_eq!(msgs.len(), out.len());
    let Some(first) = msgs.first() else { return };
    let len = first.len();
    assert!(
        msgs.iter().all(|m| m.len() == len),
        "messages differ in length"
    );
    let full = len / RATE;
    let rem = len % RATE;
    for (group, out) in msgs.chunks(W).zip(out.chunks_mut(W)) {
        // State and last block are cache-line aligned: unaligned, the kernels' vector loads and
        // 8-byte gathers split cache lines and speed depended on where the stack happened to be.
        let mut st = Aligned([[0; W]; 25]);
        let st = &mut st.0;
        let mut p = [first.as_ptr(); W];
        for (p, m) in p.iter_mut().zip(group) {
            *p = m.as_ptr();
        }
        for b in 0..full {
            // SAFETY: block b of every message (idle lanes reuse message 0) is in bounds.
            unsafe { kernel(st, &p.map(|p| p.add(b * RATE))) };
        }
        let mut last = Aligned([[0u8; RATE]; W]);
        let last = &mut last.0;
        for (blk, m) in last.iter_mut().zip(group) {
            blk[..rem].copy_from_slice(&m[full * RATE..]);
            blk[rem] ^= 0x01;
            blk[RATE - 1] ^= 0x80;
        }
        // SAFETY: each pointer is a 136-byte stack block.
        unsafe { kernel(st, &std::array::from_fn(|k| last[k].as_ptr())) };
        for (k, o) in out.iter_mut().enumerate() {
            for w in 0..4 {
                o[8 * w..8 * w + 8].copy_from_slice(&st[w][k].to_le_bytes());
            }
        }
    }
}
