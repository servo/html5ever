use criterion::{criterion_group, criterion_main, Bencher, Criterion};
use tendril::futf::classify;

static TEXT: &str = "
    All human beings are born free and equal in dignity and rights.
    They are endowed with reason and conscience and should act
    towards one another in a spirit of brotherhood.

    Minden emberi lény szabadon születik és egyenlő méltósága és
    joga van. Az emberek, ésszel és lelkiismerettel bírván,
    egymással szemben testvéri szellemben kell hogy viseltessenek.

    เราทุกคนเกิดมาอย่างอิสระ เราทุกคนมีความคิดและความเข้าใจเป็นของเราเอง
    เราทุกคนควรได้รับการปฏิบัติในทางเดียวกัน.

    모든 인간은 태어날 때부터 자유로우며 그 존엄과 권리에 있어
    동등하다. 인간은 천부적으로 이성과 양심을 부여받았으며 서로
    형제애의 정신으로 행동하여야 한다.

    ro remna cu se jinzi co zifre je simdu'i be le ry. nilselsi'a
    .e lei ry. selcru .i ry. se menli gi'e se sezmarde .i .ei
    jeseki'ubo ry. simyzu'e ta'i le tunba

    ᏂᎦᏓ ᎠᏂᏴᏫ ᏂᎨᎫᏓᎸᎾ ᎠᎴ ᎤᏂᏠᏱ ᎤᎾᏕᎿ ᏚᏳᎧᏛ ᎨᏒᎢ. ᎨᏥᏁᎳ ᎤᎾᏓᏅᏖᏗ ᎠᎴ ᎤᏃᏟᏍᏗ
    ᎠᎴ ᏌᏊ ᎨᏒ ᏧᏂᎸᏫᏍᏓᏁᏗ ᎠᎾᏟᏅᏢ ᎠᏓᏅᏙ ᎬᏗ.";

// random
static IXES: &[usize] = &[
    778, 156, 87, 604, 1216, 365, 884, 311, 469, 515, 709, 162, 871, 206, 634, 442,
];

static BOUNDARY: &[bool] = &[
    false, true, true, false, false, true, true, true, true, false, false, true, true, true, false,
    false,
];

fn std_utf8_check(b: &mut Bencher) {
    b.iter(|| {
        assert!(IXES
            .iter()
            .zip(BOUNDARY.iter())
            .all(|(&ix, &expect)| { expect == TEXT.is_char_boundary(ix) }));
    });
}

// We don't expect to be as fast as is_char_boundary, because we provide more
// information. But we shouldn't be tremendously slower, either. A factor of
// 5-10 is expected on this text.
fn futf_check(b: &mut Bencher) {
    b.iter(|| {
        assert!(IXES.iter().zip(BOUNDARY.iter()).all(|(&ix, &expect)| {
            expect == (classify(TEXT.as_bytes(), ix).unwrap().rewind == 0)
        }));
    });
}

fn tendril_benchmarks(c: &mut Criterion) {
    c.bench_function("std_utf8_check", std_utf8_check);
    c.bench_function("futf_check", futf_check);
}

criterion_group!(benches, tendril_benchmarks);
criterion_main!(benches);
