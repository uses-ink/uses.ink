//! One drawing of every kind, compared with `fixture/golden.out`: the same bytes natively and on
//! wasm (`cargo test -p freehand --target wasm32-wasip2`). `FREEHAND_BLESS=1` rewrites the
//! fixture, natively only.

#![allow(clippy::unwrap_used, reason = "tests stop at the first surprise")]

use std::fmt::Write as _;

use freehand::{Centreline, Hand, Highlighter, Mulberry, Pencil, Point, Pressured, Seed, mark};

fn drawing() -> String {
    let mut out = String::new();
    let lines = [
        (
            "straight",
            Centreline::straight(Point::new(3.0, 6.0), Point::new(290.0, 5.0), &mut Mulberry::new(Seed::of("s"))),
        ),
        ("segment", Centreline::segment(Point::new(35.0, 90.0), Point::new(22.0, 116.0))),
        ("quad", Centreline::quad(Point::new(27.0, 35.0), Point::new(36.0, 44.0), Point::new(45.0, 34.0), 24)),
        ("ellipse", Centreline::ellipse(Point::new(16.0, 16.0), Point::new(12.0, 11.0), -1.2, 1.07, 48)),
        ("dot", Centreline::through([Point::new(29.0, 26.0), Point::new(30.0, 26.0)])),
        ("chain", Centreline::chain(&[Point::new(60.0, 44.0), Point::new(52.0, 62.0), Point::new(70.0, 54.0)])),
    ];
    for (name, line) in &lines {
        let traced = Hand::default().trace(line, &mut Mulberry::new(Seed::of(name)));
        writeln!(out, "{name} {}", Pencil::new(3.2).outline(&traced)).unwrap();
    }
    let one = [Pressured::new(Point::new(4.0, 4.0), 0.5)];
    writeln!(out, "single {}", Pencil::new(3.0).outline(&one)).unwrap();
    let corners = [Point::new(4.0, 8.0), Point::new(296.0, 4.0), Point::new(294.0, 34.0), Point::new(6.0, 36.0)];
    writeln!(out, "fill {}", Highlighter::default().fill(&corners, &mut Mulberry::new(Seed::of("hl")))).unwrap();
    writeln!(out, "underline {}", mark::underline(120.0, 20.0, 3.0, "u")).unwrap();
    writeln!(out, "box-fill {}", mark::fill(Point::new(0.0, 0.0), Point::new(80.0, 30.0), Point::new(5.0, 4.0), "b"))
        .unwrap();
    let ring = Centreline::ellipse(Point::new(20.0, 20.0), Point::new(10.0, 9.0), 0.0, 1.0, 32);
    for (i, arc) in mark::drawn(&ring, Hand::default(), 3.2, "loop").iter().enumerate() {
        writeln!(out, "loop{i} {arc}").unwrap();
    }
    out
}

#[test]
fn golden() {
    let drawn = drawing();
    if std::env::var_os("FREEHAND_BLESS").is_some() {
        std::fs::write(concat!(env!("CARGO_MANIFEST_DIR"), "/fixture/golden.out"), &drawn).unwrap();
        return;
    }
    assert_eq!(drawn, include_str!("../fixture/golden.out"));
}

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
            assert_eq!(Seed::of(name).and_number(i), Seed::of(&format!("{name}{i}")));
            assert_eq!(Seed::of(name).and("f").and_number(i), Seed::of(&format!("{name}f{i}")));
        }
    }
}
