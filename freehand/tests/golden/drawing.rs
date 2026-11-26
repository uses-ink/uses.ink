//! One drawing of every kind of line, pencil and highlighter.

use std::fmt::Write as _;

use freehand::mark::{Drawn, Fill, Underline};
use freehand::{Centreline, Ellipse, Hand, Highlighter, Mark, Mulberry, Pencil, Point, Pressured, Rectangle, Seed};

fn chance(name: &str) -> Mulberry {
    Mulberry::from(Seed::of(name))
}

fn lines() -> [(&'static str, Centreline); 6] {
    [
        ("straight", Centreline::straight(Point::new(3.0, 6.0), Point::new(290.0, 5.0), &mut chance("s"))),
        ("segment", Centreline::segment(Point::new(35.0, 90.0), Point::new(22.0, 116.0))),
        ("quad", Centreline::quad(Point::new(27.0, 35.0), Point::new(36.0, 44.0), Point::new(45.0, 34.0), 24)),
        (
            "ellipse",
            Centreline::from(Ellipse {
                centre: Point::new(16.0, 16.0),
                radius: Point::new(12.0, 11.0),
                from: -1.2,
                turns: 1.07,
                count: 48,
            }),
        ),
        ("dot", [Point::new(29.0, 26.0), Point::new(30.0, 26.0)].into_iter().collect()),
        ("chain", Centreline::chain([Point::new(60.0, 44.0), Point::new(52.0, 62.0), Point::new(70.0, 54.0)])),
    ]
}

pub(crate) fn drawing() -> String {
    let mut out = String::new();
    for (name, line) in &lines() {
        let traced = Hand::default().trace(line, &mut chance(name));
        writeln!(out, "{name} {}", Pencil::new(3.2).outline(&traced)).unwrap();
    }
    let one = [Pressured::new(Point::new(4.0, 4.0), 0.5)];
    writeln!(out, "single {}", Pencil::new(3.0).outline(&one)).unwrap();
    let corners = [Point::new(4.0, 8.0), Point::new(296.0, 4.0), Point::new(294.0, 34.0), Point::new(6.0, 36.0)];
    writeln!(out, "fill {}", Highlighter::default().fill(&corners, &mut chance("hl"))).unwrap();
    let hand = Hand::default();
    let underline = Underline { width: 120.0, y: 20.0 };
    writeln!(out, "underline {}", underline.draw(&hand, &Pencil::new(3.0), &mut chance("u"))).unwrap();
    let fill = Fill {
        behind: Rectangle { top_left: Point::new(0.0, 0.0), bottom_right: Point::new(80.0, 30.0) },
        shift: Point::new(5.0, 4.0),
    };
    writeln!(out, "box-fill {}", fill.draw(&hand, &Highlighter::default(), &mut chance("b"))).unwrap();
    let ring = Centreline::from(Ellipse {
        centre: Point::new(20.0, 20.0),
        radius: Point::new(10.0, 9.0),
        from: 0.0,
        turns: 1.0,
        count: 32,
    });
    for (i, arc) in Drawn(&ring).draw(&hand, &Pencil::new(3.2), &mut chance("loop")).into_iter().enumerate() {
        writeln!(out, "loop{i} {arc}").unwrap();
    }
    out
}
