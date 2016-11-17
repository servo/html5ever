#![feature(test)]
#![allow(deprecated)]

extern crate html5ever_atoms;
extern crate string_cache;
extern crate test;

use std::hash::{Hasher, SipHasher};
use std::ops::BitXor;

fn atoms() -> &'static [&'static str] {
    <html5ever_atoms::LocalNameStaticSet as string_cache::StaticAtomSet>::get().atoms
}

#[bench]
fn bench_siphash(bencher: &mut test::Bencher) {
    bencher.iter(|| {
        for atom in atoms() {
            let mut hasher = SipHasher::new();
            hasher.write(atom.as_bytes());
            test::black_box(hasher.finish());
        }
    })
}

#[bench]
fn bench_fnv(bencher: &mut test::Bencher) {
    bencher.iter(|| {
        for atom in atoms() {
            test::black_box(fnv(atom));
        }
    })
}

#[bench]
fn bench_fnv32(bencher: &mut test::Bencher) {
    bencher.iter(|| {
        for atom in atoms() {
            test::black_box(fnv32(atom));
        }
    })
}

#[bench]
fn bench_fx_bytes(bencher: &mut test::Bencher) {
    bencher.iter(|| {
        for atom in atoms() {
            test::black_box(fx_bytes(atom));
        }
    })
}

#[bench]
fn bench_fx_u64(bencher: &mut test::Bencher) {
    bencher.iter(|| {
        for atom in atoms() {
            test::black_box(fx_u64(atom));
        }
    })
}

// http://www.isthe.com/chongo/tech/comp/fnv/index.html
fn fnv(s: &str) -> u64 {
    let mut h = 0xcbf29ce484222325;
    for b in s.bytes() {
        h = (h ^ (b as u64)).wrapping_mul(0x100000001b3);
    }
    h
}

fn fnv32(s: &str) -> u32 {
    let mut h = 0x811c9dc5;
    for b in s.bytes() {
        h = (h ^ (b as u32)).wrapping_mul(0x1000193);
    }
    h
}

// https://github.com/rust-lang/rust/pull/37229
fn fx_bytes(s: &str) -> u64 {
    let mut h: u64 = 0;
    for b in s.bytes() {
        h = h.rotate_left(5).bitxor(b as u64).wrapping_mul(0x517cc1b727220a95);
    }
    h
}

fn fx_u64(s: &str) -> u64 {
    let mut h: u64 = 0;
    let s = s.as_bytes();
    let n_chunks = s.len() / 8;
    for chunk in 0..n_chunks {
        let start = chunk * 8;
        let word =
            (s[start + 0] as u64) << 0 |
            (s[start + 1] as u64) << 8 |
            (s[start + 2] as u64) << 16 |
            (s[start + 3] as u64) << 24 |
            (s[start + 4] as u64) << 32 |
            (s[start + 5] as u64) << 40 |
            (s[start + 6] as u64) << 48 |
            (s[start + 7] as u64) << 56;
        h = h.rotate_left(5).bitxor(word).wrapping_mul(0x517cc1b727220a95);
    }
    for &b in &s[n_chunks * 8..] {
        h = h.rotate_left(5).bitxor(b as u64).wrapping_mul(0x517cc1b727220a95);
    }
    h
}
