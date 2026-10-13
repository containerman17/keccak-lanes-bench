//! Own 4-way (AVX2) and 8-way (AVX-512) Keccak-f[1600] on stable `std::arch` intrinsics.
//! One SIMD element holds the same state lane of a different message.

use crate::sponge::{self, RATE, State};
use std::arch::x86_64::*;

const RC: [u64; 24] = [
    0x0000000000000001,
    0x0000000000008082,
    0x800000000000808A,
    0x8000000080008000,
    0x000000000000808B,
    0x0000000080000001,
    0x8000000080008081,
    0x8000000000008009,
    0x000000000000008A,
    0x0000000000000088,
    0x0000000080008009,
    0x000000008000000A,
    0x000000008000808B,
    0x800000000000008B,
    0x8000000000008089,
    0x8000000000008003,
    0x8000000000008002,
    0x8000000000000080,
    0x000000000000800A,
    0x800000008000000A,
    0x8000000080008081,
    0x8000000000008080,
    0x0000000080000001,
    0x8000000080008008,
];

/// Rotation of lane x + 5y.
const RHO: [i64; 25] = [
    0, 1, 62, 28, 27, 36, 44, 6, 55, 20, 3, 10, 43, 25, 39, 41, 45, 15, 21, 8, 18, 2, 61, 56, 14,
];

/// Pi moves lane (x, y) to (y, 2x + 3y).
const PI: [usize; 25] = {
    let mut p = [0; 25];
    let mut i = 0;
    while i < 25 {
        let (x, y) = (i % 5, i / 5);
        p[i] = y + 5 * ((2 * x + 3 * y) % 5);
        i += 1;
    }
    p
};

/// Repeats `$body` with `$i` bound to each literal, so every array index is a constant and
/// the state stays in registers (a plain `for` over 25 lanes is not unrolled by LLVM).
macro_rules! unroll {
    ($i:ident in [$($n:literal),*] $body:block) => { $( { let $i: usize = $n; $body } )* };
}

/// 24 rounds over `a: [V; 25]`, written once for both vector widths. The op macros are
/// defined inside each kernel so they expand under that kernel's target features.
macro_rules! keccak_f1600 {
    ($a:ident) => {
        for rc in RC {
            let mut c = [$a[0]; 5];
            unroll!(x in [0, 1, 2, 3, 4] {
                c[x] = xor5!($a[x], $a[x + 5], $a[x + 10], $a[x + 15], $a[x + 20]);
            });
            let mut d = c;
            unroll!(x in [0, 1, 2, 3, 4] {
                d[x] = xor!(c[(x + 4) % 5], rol!(c[(x + 1) % 5], 1));
            });
            let mut b = $a;
            unroll!(i in [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24] {
                b[PI[i]] = rol!(xor!($a[i], d[i % 5]), RHO[i]);
            });
            unroll!(y in [0, 5, 10, 15, 20] {
                unroll!(x in [0, 1, 2, 3, 4] {
                    $a[x + y] = chi!(b[x + y], b[(x + 1) % 5 + y], b[(x + 2) % 5 + y]);
                });
            });
            $a[0] = xor!($a[0], splat!(rc));
        }
    };
}

#[target_feature(enable = "avx2")]
unsafe fn kernel4(st: &mut State<4>, p: &[*const u8; 4]) {
    macro_rules! splat {
        ($n:expr) => {
            _mm256_set1_epi64x($n as i64)
        };
    }
    macro_rules! xor {
        ($a:expr, $b:expr) => {
            _mm256_xor_si256($a, $b)
        };
    }
    macro_rules! xor5 {
        ($a:expr, $b:expr, $c:expr, $d:expr, $e:expr) => {
            xor!(xor!(xor!($a, $b), xor!($c, $d)), $e)
        };
    }
    // Constant counts fold into immediate shifts. A count of 64 shifts to zero, so rol 0 is exact.
    macro_rules! rol {
        ($v:expr, $n:expr) => {{
            let v = $v;
            _mm256_or_si256(
                _mm256_sllv_epi64(v, splat!($n)),
                _mm256_srlv_epi64(v, splat!(64 - $n)),
            )
        }};
    }
    macro_rules! chi {
        ($a:expr, $b:expr, $c:expr) => {
            xor!($a, _mm256_andnot_si256($b, $c))
        };
    }

    let mut a = [_mm256_setzero_si256(); 25];
    for i in 0..25 {
        a[i] = unsafe { _mm256_loadu_si256(st[i].as_ptr().cast()) };
    }
    // Gather word w of every message as base + offset, the offsets are pointer differences.
    let base = p[0].cast::<i64>();
    let off = |k: usize| (p[k] as i64).wrapping_sub(p[0] as i64);
    let idx = _mm256_set_epi64x(off(3), off(2), off(1), 0);
    for w in 0..RATE / 8 {
        a[w] = xor!(a[w], unsafe {
            _mm256_i64gather_epi64::<1>(base.wrapping_add(w), idx)
        });
    }
    keccak_f1600!(a);
    for i in 0..25 {
        unsafe { _mm256_storeu_si256(st[i].as_mut_ptr().cast(), a[i]) };
    }
}

#[target_feature(enable = "avx512f")]
unsafe fn kernel8(st: &mut State<8>, p: &[*const u8; 8]) {
    macro_rules! splat {
        ($n:expr) => {
            _mm512_set1_epi64($n as i64)
        };
    }
    macro_rules! xor {
        ($a:expr, $b:expr) => {
            _mm512_xor_si512($a, $b)
        };
    }
    // vpternlogq: 0x96 is a ^ b ^ c, 0xD2 is a ^ (!b & c).
    macro_rules! xor5 {
        ($a:expr, $b:expr, $c:expr, $d:expr, $e:expr) => {
            _mm512_ternarylogic_epi64::<0x96>(_mm512_ternarylogic_epi64::<0x96>($a, $b, $c), $d, $e)
        };
    }
    macro_rules! rol {
        ($v:expr, $n:expr) => {
            _mm512_rolv_epi64($v, splat!($n))
        };
    }
    macro_rules! chi {
        ($a:expr, $b:expr, $c:expr) => {
            _mm512_ternarylogic_epi64::<0xD2>($a, $b, $c)
        };
    }

    let mut a = [_mm512_setzero_si512(); 25];
    for i in 0..25 {
        a[i] = unsafe { _mm512_loadu_si512(st[i].as_ptr().cast()) };
    }
    let base = p[0].cast::<i64>();
    let off = |k: usize| (p[k] as i64).wrapping_sub(p[0] as i64);
    let idx = _mm512_set_epi64(off(7), off(6), off(5), off(4), off(3), off(2), off(1), 0);
    for w in 0..RATE / 8 {
        a[w] = xor!(a[w], unsafe {
            _mm512_i64gather_epi64::<1>(idx, base.wrapping_add(w))
        });
    }
    keccak_f1600!(a);
    for i in 0..25 {
        unsafe { _mm512_storeu_si512(st[i].as_mut_ptr().cast(), a[i]) };
    }
}

pub fn hash_avx2(msgs: &[&[u8]], out: &mut [[u8; 32]]) {
    assert!(is_x86_feature_detected!("avx2"));
    // SAFETY: AVX2 checked above.
    unsafe { sponge::hash::<4>(msgs, out, kernel4) }
}

pub fn hash_avx512(msgs: &[&[u8]], out: &mut [[u8; 32]]) {
    assert!(is_x86_feature_detected!("avx512f"));
    // SAFETY: AVX-512F checked above.
    unsafe { sponge::hash::<8>(msgs, out, kernel8) }
}

/// Groups of 8 on AVX-512 (else 4 on AVX2), but a lone message goes to sha3: on the
/// benchmark CPU one 8-lane call with a single message costs more than one scalar hash,
/// while two messages are already cheaper on 8 lanes than on 4 lanes or scalar.
pub fn hash_auto(msgs: &[&[u8]], out: &mut [[u8; 32]]) {
    let (w, wide): (usize, crate::HashFn) = if is_x86_feature_detected!("avx512f") {
        (8, hash_avx512)
    } else if is_x86_feature_detected!("avx2") {
        (4, hash_avx2)
    } else {
        (1, crate::sha3_stock)
    };
    for (m, o) in msgs.chunks(w).zip(out.chunks_mut(w)) {
        if m.len() == 1 {
            crate::sha3_stock(m, o)
        } else {
            wide(m, o)
        }
    }
}
