//! The reader theme's marks. The geometry first matched the mock's JS pen (`gen-marks.mjs`) byte
//! for byte; since 0038 the paths are written shorter, and `fixture/marks.snap` is the reference.

use std::fmt::Write as _;

use freehand::mark::{Frame, Line, Patch, Ring};
use freehand::{Ellipse, Hand, Highlighter, Mark, Mulberry, Paths, Pencil, Point, Rectangle, Seed};

fn at(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn ellipse(centre: Point, radius: Point, turns: (f64, f64), count: u32) -> Ellipse {
    let (from, turns) = turns;
    Ellipse { centre, radius, from, turns, count }
}

/// `mark` drawn by the page's hand with `tool`, seeded by `name`.
fn drawn(mark: &impl Mark, tool: &impl freehand::Tool, name: &str) -> Paths {
    mark.draw(&Hand::default(), tool, &mut Mulberry::from(Seed::of(name)))
}

pub(crate) fn drawing() -> String {
    let line = |from, to| Line { from, to };
    let marker = Highlighter::default();
    let mut marks = vec![
        ("rule".to_owned(), drawn(&line(at(6.0, 6.0), at(790.0, 7.0)), &Pencil::new(2.6), "rule")),
        ("underline-pen".to_owned(), drawn(&line(at(3.0, 4.0), at(292.0, 5.0)), &Pencil::new(2.4), "ul-pen")),
        ("underline-ink".to_owned(), drawn(&line(at(3.0, 6.0), at(290.0, 5.0)), &Pencil::new(3.4), "ul-ink")),
    ];
    let frame = Frame { around: Rectangle { top_left: at(12.0, 12.0), bottom_right: at(588.0, 388.0) }, slack: 3.0 };
    let frame = drawn(&frame, &Pencil::new(2.6), "frame");
    marks.extend(frame.into_iter().enumerate().map(|(i, d)| (format!("frame{i}"), d.into())));
    let circle = Ring(ellipse(at(16.0, 16.0), at(12.0, 11.0), (-1.2, 1.075), 48));
    let bullet = Ring(ellipse(at(10.0, 10.0), at(5.5, 5.0), (0.0, 1.1), 40));
    marks.extend([
        ("circle".to_owned(), drawn(&circle, &Pencil::new(2.2), "circle")),
        (
            "tape".to_owned(),
            drawn(&Patch([at(2.0, 4.0), at(98.0, 2.0), at(97.0, 27.0), at(3.0, 28.0)]), &marker, "tape"),
        ),
        (
            "highlight".to_owned(),
            drawn(&Patch([at(4.0, 8.0), at(296.0, 4.0), at(294.0, 34.0), at(6.0, 36.0)]), &marker, "hl"),
        ),
        ("bullet".to_owned(), drawn(&bullet, &Pencil::new(2.4), "bullet")),
    ]);
    let mut out = String::new();
    for (name, d) in &marks {
        writeln!(out, "{name} {d}").unwrap();
    }
    out
}
