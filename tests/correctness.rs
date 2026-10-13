use keccak_lanes_bench::{impls, tiny_keccak};
use std::time::{Duration, Instant};

fn hex(h: &[u8; 32]) -> String {
    h.iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn published_vectors() {
    let fox = b"The quick brown fox jumps over the lazy dog";
    let cases: [(&[u8], &str); 3] = [
        (
            b"",
            "c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470",
        ),
        (
            b"abc",
            "4e03657aea45a94fc7d47ba826c8d667c0d1e6e33a64a036ec44f58fa12d6c45",
        ),
        (
            fox,
            "4d741b6f1eb29cb2a9b9911c82f56fa8d73b04959d3d9d222895df6c0b28aa15",
        ),
    ];
    for imp in impls() {
        for (msg, want) in cases {
            for batch in [1, 3, 9] {
                let msgs = vec![msg; batch];
                let mut out = vec![[0u8; 32]; batch];
                (imp.hash)(&msgs, &mut out);
                for h in &out {
                    assert_eq!(hex(h), want, "{} batch {batch}", imp.name);
                }
            }
        }
    }
}

/// Lengths 0..=300, then random lengths 0..=5120 and batch sizes 1..=20, messages at random (unaligned) offsets,
/// every implementation against tiny-keccak. Runs FUZZ_SECS seconds (default 3).
#[test]
fn randomized_against_tiny_keccak() {
    let secs = std::env::var("FUZZ_SECS").map_or(3, |s| s.parse().unwrap());
    let deadline = Instant::now() + Duration::from_secs(secs);
    let mut seed = 0x9E37_79B9_7F4A_7C15u64;
    let mut rng = move || {
        // splitmix64
        seed = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = seed;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    };
    let pool: Vec<u8> = (0..200_000).map(|_| rng() as u8).collect();
    let impls = impls();
    let mut cases = 0u64;
    while Instant::now() < deadline {
        // Every length up to 300 first (block boundaries 135, 136, 271, 272), then random.
        let len = if cases <= 300 {
            cases as usize
        } else {
            (rng() % 5121) as usize
        };
        let batch = (rng() % 20 + 1) as usize;
        let msgs: Vec<&[u8]> = (0..batch)
            .map(|_| {
                let at = (rng() % (pool.len() - len) as u64) as usize;
                &pool[at..at + len]
            })
            .collect();
        let mut want = vec![[0u8; 32]; batch];
        tiny_keccak(&msgs, &mut want);
        for imp in &impls {
            let mut got = vec![[0u8; 32]; batch];
            (imp.hash)(&msgs, &mut got);
            assert_eq!(got, want, "{} len {len} batch {batch}", imp.name);
        }
        cases += 1;
    }
    eprintln!(
        "{cases} random cases x {} implementations matched",
        impls.len()
    );
}
