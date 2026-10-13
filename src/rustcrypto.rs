//! RustCrypto `keccak` 0.1 `simd` permutations (core::simd) inside the shared sponge.
//! The crate only provides the permutation, so absorbing is plain scalar code here.

use crate::sponge::{self, RATE, State};
use core::simd::Simd;

macro_rules! lanes {
    ($kernel:ident, $hash:ident, $w:literal, $f:path) => {
        unsafe fn $kernel(st: &mut State<$w>, p: &[*const u8; $w]) {
            for (k, p) in p.iter().enumerate() {
                for w in 0..RATE / 8 {
                    // SAFETY: the sponge passes pointers valid for RATE bytes.
                    let word = unsafe { p.add(8 * w).cast::<[u8; 8]>().read_unaligned() };
                    st[w][k] ^= u64::from_le_bytes(word);
                }
            }
            let mut v = st.map(Simd::from_array);
            $f(&mut v);
            *st = v.map(Simd::to_array);
        }

        pub fn $hash(msgs: &[&[u8]], out: &mut [[u8; 32]]) {
            // SAFETY: portable code, no CPU feature needed.
            unsafe { sponge::hash::<$w>(msgs, out, $kernel) }
        }
    };
}

lanes!(kernel2, hash_x2, 2, keccak01::simd::f1600x2);
lanes!(kernel4, hash_x4, 4, keccak01::simd::f1600x4);
lanes!(kernel8, hash_x8, 8, keccak01::simd::f1600x8);
