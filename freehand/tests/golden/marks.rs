//! The reader theme's marks. The geometry first matched the mock's JS pen (`gen-marks.mjs`) byte
//! for byte; since 0038 the paths are written shorter, and `fixture/marks.snap` is the reference.

use std::fmt::Write as _;

use freehand::{Ellipse, Point, Rectangle, mark};

fn at(x: f64, y: f64) -> Point {
    Point::new(x, y)
}

fn ellipse(centre: Point, radius: Point, turns: (f64, f64), count: u32) -> Ellipse {
    let (from, turns) = turns;
    Ellipse { centre, radius, from, turns, count }
}

pub(crate) fn drawing() -> String {
    let mut marks = vec![
        ("rule".to_owned(), mark::line(at(6.0, 6.0), at(790.0, 7.0), 2.6, "rule")),
        ("underline-pen".to_owned(), mark::line(at(3.0, 4.0), at(292.0, 5.0), 2.4, "ul-pen")),
        ("underline-ink".to_owned(), mark::line(at(3.0, 6.0), at(290.0, 5.0), 3.4, "ul-ink")),
    ];
    let frame = mark::frame(Rectangle { top_left: at(12.0, 12.0), bottom_right: at(588.0, 388.0) }, 3.0, 2.6, "frame");
    marks.extend(frame.into_iter().enumerate().map(|(i, d)| (format!("frame{i}"), d)));
    marks.extend([
        ("circle".to_owned(), mark::ring(ellipse(at(16.0, 16.0), at(12.0, 11.0), (-1.2, 1.075), 48), 2.2, "circle")),
        ("tape".to_owned(), mark::patch(&[at(2.0, 4.0), at(98.0, 2.0), at(97.0, 27.0), at(3.0, 28.0)], "tape")),
        ("highlight".to_owned(), mark::patch(&[at(4.0, 8.0), at(296.0, 4.0), at(294.0, 34.0), at(6.0, 36.0)], "hl")),
        ("bullet".to_owned(), mark::ring(ellipse(at(10.0, 10.0), at(5.5, 5.0), (0.0, 1.1), 40), 2.4, "bullet")),
    ]);
    let mut out = String::new();
    for (name, d) in &marks {
        writeln!(out, "{name} {d}").unwrap();
    }
    out
}
