// // Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// // https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// // <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// // option. This file may not be copied, modified, or distributed
// // except according to those terms.

#![allow(clippy::manual_pattern_char_comparison)]

extern crate criterion;
extern crate tendril;
use std::collections::{hash_map::Entry, HashMap};

use criterion::{criterion_group, criterion_main, Bencher, Criterion};
use tendril::StrTendril;

static EN_1: &str = "Days turn to nights turn to paper into rocks into plastic";

static EN_2: &str = "Here the notes in my laboratory journal cease. I was able to write the last \
       words only with great effort. By now it was already clear to me that LSD had \
       been the cause of the remarkable experience of the previous Friday, for the \
       altered perceptions were of the same type as before, only much more intense. I \
       had to struggle to speak intelligibly. I asked my laboratory assistant, who was \
       informed of the self-experiment, to escort me home. We went by bicycle, no \
       automobile being available because of wartime restrictions on their use. On the \
       way home, my condition began to assume threatening forms. Everything in my \
       field of vision wavered and was distorted as if seen in a curved mirror. I also \
       had the sensation of being unable to move from the spot. Nevertheless, my \
       assistant later told me that we had traveled very rapidly. Finally, we arrived \
       at home safe and sound, and I was just barely capable of asking my companion to \
       summon our family doctor and request milk from the neighbors.\n\n\
       In spite of my delirious, bewildered condition, I had brief periods of clear \
       and effective thinking—and chose milk as a nonspecific antidote for poisoning.";

static KR_1: &str = "러스트(Rust)는 모질라(mozilla.org)에서 개발하고 있는, 메모리-안전하고 병렬 \
       프로그래밍이 쉬운 차세대 프로그래밍 언어입니다. 아직 \
       개발 단계이며 많은 기능이 구현 중으로, MIT/Apache2 라이선스로 배포됩니다.";

static HTML_KR_1: &str = "<p>러스트(<a href=\"http://rust-lang.org\">Rust</a>)는 모질라(<a href=\"\
       https://www.mozilla.org/\">mozilla.org</a>)에서 개발하고 있는, \
       메모리-안전하고 병렬 프로그래밍이 쉬운 차세대 프로그래밍 언어입니다. \
       아직 개발 단계이며 많은 기능이 구현 중으로, MIT/Apache2 라이선스로 배포됩니다.</p>";

const SMALL_SIZE: usize = 65536;
const LARGE_SIZE: usize = 1 << 20;

fn index_words_string(input: &str) -> HashMap<char, Vec<String>> {
    let mut index = HashMap::new();
    for word in input.split(|c| c == ' ') {
        if word.is_empty() {
            continue;
        }
        let word = word.to_owned();
        match index.entry(word.chars().next().unwrap()) {
            Entry::Occupied(mut e) => {
                let x: &mut Vec<String> = e.get_mut();
                x.push(word);
            },
            Entry::Vacant(e) => {
                e.insert(vec![word]);
            },
        }
    }
    index
}

fn index_words_tendril(input: &StrTendril) -> HashMap<char, Vec<StrTendril>> {
    let mut index = HashMap::new();
    let mut t = input.clone();
    loop {
        match t.pop_front_char_run(|c| c != ' ') {
            None => return index,
            Some((_, false)) => (),
            Some((word, true)) => match index.entry(word.chars().next().unwrap()) {
                Entry::Occupied(mut e) => {
                    e.get_mut().push(word);
                },
                Entry::Vacant(e) => {
                    e.insert(vec![word]);
                },
            },
        }
    }
}

fn test_correctness(txt: &str) {
    use std::borrow::ToOwned;
    use tendril::SliceExt;

    let input_string = txt.to_owned();
    let count_s = index_words_string(&input_string);
    let mut keys: Vec<char> = count_s.keys().cloned().collect();
    keys.sort();

    let input_tendril = txt.to_tendril();
    let count_t = index_words_tendril(&input_tendril);
    let mut keys_t: Vec<char> = count_t.keys().cloned().collect();
    keys_t.sort();

    assert_eq!(keys, keys_t);

    for k in &keys {
        let vs = &count_s[k];
        let vt = &count_t[k];
        assert_eq!(vs.len(), vt.len());
        assert!(vs.iter().zip(vt.iter()).all(|(s, t)| **s == **t));
    }
}

fn index_words_small_string(b: &mut Bencher, txt: &str) {
    let mut s = String::new();
    while s.len() < SMALL_SIZE {
        s.push_str(txt);
    }
    b.iter(|| index_words_string(&s));
}

fn index_words_small_tendril(b: &mut Bencher, txt: &str) {
    let mut t = StrTendril::new();
    while t.len() < SMALL_SIZE {
        t.push_slice(txt);
    }
    b.iter(|| index_words_tendril(&t));
}

fn index_words_big_string(b: &mut Bencher, txt: &str) {
    let mut s = String::new();
    while s.len() < LARGE_SIZE {
        s.push_str(txt);
    }
    b.iter(|| index_words_string(&s));
}

fn index_words_big_tendril(b: &mut Bencher, txt: &str) {
    let mut t = StrTendril::new();
    while t.len() < LARGE_SIZE {
        t.push_slice(txt);
    }
    b.iter(|| index_words_tendril(&t));
}

fn run_bench_group(c: &mut Criterion, group_name: &str, txt: &str) {
    let mut group = c.benchmark_group(group_name);

    test_correctness(txt);

    group.bench_with_input("index_words_small_string", txt, index_words_small_string);
    group.bench_with_input("index_words_small_tendril", txt, index_words_small_tendril);
    group.bench_with_input("index_words_big_string", txt, index_words_big_string);
    group.bench_with_input("index_words_big_tendril", txt, index_words_big_tendril);
}

fn tendril_benchmarks(c: &mut Criterion) {
    run_bench_group(c, "en_1", EN_1);
    run_bench_group(c, "en_2", EN_2);
    run_bench_group(c, "kr_1", KR_1);
    run_bench_group(c, "html_kr_1", HTML_KR_1);
}

criterion_group!(benches, tendril_benchmarks);
criterion_main!(benches);
