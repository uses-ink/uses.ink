//! Seeds: FNV-1a over UTF-16, continued one piece at a time.

use freehand::{Chance, Mulberry, Seed};

#[test]
fn seeds_are_fnv_over_utf16() {
    assert_eq!(Seed::of("").0, 2_166_136_261);
    assert_eq!(Seed::of("logo0"), Seed::of("logo0"));
    assert_ne!(Seed::of("logo0"), Seed::of("logo1"));
}

/// A continued seed is the seed of the whole text, so `{name}{i}` and `{name}f{i}` seeds built
/// by continuing are the ones built from the spelled-out name.
#[test]
fn continued_seeds_are_seeds_of_the_whole_text() {
    for name in ["", "logo", "frame", "é字🖊"] {
        for i in [0, 1, 9, 10, 99, 12_345, usize::MAX] {
            assert_eq!(Seed::of(name).and(i), Seed::of(format!("{name}{i}")));
            assert_eq!(Seed::of(name).and("f").and(i), Seed::of(format!("{name}f{i}")));
        }
    }
}

/// A fork draws from its own seed, whatever its parent has drawn.
#[test]
fn forks_draw_from_the_continued_seed() {
    let mut parent = Mulberry::from(Seed::of("frame"));
    let before = parent.fork(2).draw();
    parent.draw();
    assert_eq!(parent.fork(2).draw(), before);
    assert_eq!(Mulberry::from(Seed::of("frame2")).draw(), before);
}
