//! Keccak-256 (Ethereum legacy padding) of a batch of equal-length messages.
//! Every implementation has the same signature: `out[i] = keccak256(msgs[i])`.
#![cfg_attr(feature = "rustcrypto-simd", feature(portable_simd))]

#[cfg(target_arch = "x86_64")]
pub mod lanes;
#[cfg(feature = "rustcrypto-simd")]
pub mod rustcrypto;
pub mod sponge;

pub type HashFn = fn(&[&[u8]], &mut [[u8; 32]]);

pub struct Impl {
    pub name: &'static str,
    pub hash: HashFn,
}

/// Stock Firewood: `sha3::Keccak256::digest`, one node per call.
pub fn sha3_stock(msgs: &[&[u8]], out: &mut [[u8; 32]]) {
    use sha3::{Digest, Keccak256};
    for (m, o) in msgs.iter().zip(out) {
        *o = Keccak256::digest(m).into();
    }
}

pub fn tiny_keccak(msgs: &[&[u8]], out: &mut [[u8; 32]]) {
    use tiny_keccak::{Hasher, Keccak};
    for (m, o) in msgs.iter().zip(out) {
        let mut h = Keccak::v256();
        h.update(m);
        h.finalize(o);
    }
}

pub fn keccak_asm(msgs: &[&[u8]], out: &mut [[u8; 32]]) {
    use keccak_asm::Keccak256;
    for (m, o) in msgs.iter().zip(out) {
        *o = Keccak256::digest(m).into();
    }
}

/// Every implementation the running CPU supports, stock first.
pub fn impls() -> Vec<Impl> {
    let mut v = vec![
        Impl {
            name: "sha3 0.12 (stock Firewood)",
            hash: sha3_stock,
        },
        Impl {
            name: "tiny-keccak 2.0",
            hash: tiny_keccak,
        },
        Impl {
            name: "keccak-asm 0.1",
            hash: keccak_asm,
        },
    ];
    #[cfg(feature = "rustcrypto-simd")]
    v.extend([
        Impl {
            name: "keccak 0.1 simd x2",
            hash: rustcrypto::hash_x2,
        },
        Impl {
            name: "keccak 0.1 simd x4",
            hash: rustcrypto::hash_x4,
        },
        Impl {
            name: "keccak 0.1 simd x8",
            hash: rustcrypto::hash_x8,
        },
    ]);
    #[cfg(target_arch = "x86_64")]
    {
        if std::is_x86_feature_detected!("avx2") {
            v.push(Impl {
                name: "own AVX2 x4",
                hash: lanes::hash_avx2,
            });
        }
        if std::is_x86_feature_detected!("avx512f") {
            v.push(Impl {
                name: "own AVX-512 x8",
                hash: lanes::hash_avx512,
            });
        }
        v.push(Impl {
            name: "own auto (x8; sha3 for a lone message)",
            hash: lanes::hash_auto,
        });
    }
    v
}
