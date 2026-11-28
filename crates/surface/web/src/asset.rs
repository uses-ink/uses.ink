//! The files the dashboard serves beside its pages (decision 0190): the three faces, the brand
//! mark, and the strokes the stylesheet borrows. Every stroke is drawn by `freehand` when it is
//! asked for, from a fixed name, so the same file comes back every time and no generated drawing
//! is committed. Boxes are border images: a large frame whose corners stay as drawn and whose
//! sides stretch along their own length, so a line keeps one width at any box size.

use std::borrow::Cow;
use std::fmt;

use freehand::mark::{Drawn, Frame as Edges, Line};
use freehand::{Centreline, Ellipse, Hand, Mark, Mulberry, Paths, Pencil, Point, Rectangle, Seed};
use token::Colour;

/// Which of three hands drew a box, so neighbouring boxes do not wobble alike.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Wobble {
    /// The first hand.
    First,
    /// The second hand.
    Second,
    /// The third hand.
    Third,
}

impl Wobble {
    /// Every hand.
    pub const ALL: [Self; 3] = [Self::First, Self::Second, Self::Third];

    const fn number(self) -> u8 {
        match self {
            Self::First => 1,
            Self::Second => 2,
            Self::Third => 3,
        }
    }
}

/// What kind of file an asset is, which is what its `Content-Type` says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Medium {
    /// A `woff2` face.
    Font,
    /// An SVG image.
    Image,
    /// A stylesheet.
    Style,
}

/// One file the dashboard serves: found by its path, answered with its bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Asset {
    /// Gaegu, regular: the hand of the whole dashboard.
    Regular,
    /// Gaegu, bold.
    Bold,
    /// Monaspace Radon: paths and code.
    Mono,
    /// The paint box and the face stacks as custom properties, from `token.toml`.
    Tokens,
    /// The brand mark.
    Mark,
    /// A box, four crossing pencil strokes.
    Outline(Wobble),
    /// A small box, for a tab, a button or a field.
    Small,
    /// A pencil line under a table row.
    Rule,
    /// A red pen line under a link.
    Underline,
    /// A smiling face for the person's own tab.
    Face,
}

impl Asset {
    /// Every asset, so a page can name them all and a test can ask each.
    pub const ALL: [Self; 12] = [
        Self::Tokens,
        Self::Regular,
        Self::Bold,
        Self::Mono,
        Self::Mark,
        Self::Outline(Wobble::First),
        Self::Outline(Wobble::Second),
        Self::Outline(Wobble::Third),
        Self::Small,
        Self::Rule,
        Self::Underline,
        Self::Face,
    ];

    /// The size, in pixels, of the square a box is drawn in, and so the slice of its corners.
    pub const SQUARE: u32 = 480;

    /// The size of the small square.
    pub const LITTLE: u32 = 96;

    /// The asset served at `file`, the last part of its path.
    pub fn named(file: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|asset| asset.file() == file)
    }

    /// The path it is served at, which the stylesheet and the pages name.
    pub const fn path(self) -> &'static str {
        match self {
            Self::Tokens => "/token.css",
            Self::Regular => "/gaegu-regular.woff2",
            Self::Bold => "/gaegu-bold.woff2",
            Self::Mono => "/monaspace-radon.woff2",
            Self::Mark => "/mark.svg",
            Self::Outline(Wobble::First) => "/outline-1.svg",
            Self::Outline(Wobble::Second) => "/outline-2.svg",
            Self::Outline(Wobble::Third) => "/outline-3.svg",
            Self::Small => "/outline-small.svg",
            Self::Rule => "/rule.svg",
            Self::Underline => "/underline.svg",
            Self::Face => "/face.svg",
        }
    }

    /// The file name it is served as: its path without the slash.
    pub fn file(self) -> &'static str {
        self.path().trim_start_matches('/')
    }

    /// What kind of file it is.
    pub const fn medium(self) -> Medium {
        match self {
            Self::Tokens => Medium::Style,
            Self::Regular | Self::Bold | Self::Mono => Medium::Font,
            _ => Medium::Image,
        }
    }

    /// Its bytes: a face or the mark as committed, a stroke as drawn now.
    pub fn bytes(self) -> Cow<'static, [u8]> {
        match self {
            Self::Tokens => Cow::Borrowed(include_bytes!("../../../core/token/token.css")),
            Self::Regular => Cow::Borrowed(include_bytes!("../assets/gaegu-regular.woff2")),
            Self::Bold => Cow::Borrowed(include_bytes!("../assets/gaegu-bold.woff2")),
            Self::Mono => Cow::Borrowed(include_bytes!("../assets/monaspace-radon.woff2")),
            Self::Mark => Cow::Borrowed(include_bytes!("../../../../brand/mark.svg")),
            Self::Outline(wobble) => Cow::Owned(Sketch::outline(wobble).to_string().into_bytes()),
            Self::Small => Cow::Owned(Sketch::small().to_string().into_bytes()),
            Self::Rule => Cow::Owned(Sketch::rule().to_string().into_bytes()),
            Self::Underline => Cow::Owned(Sketch::underline().to_string().into_bytes()),
            Self::Face => Cow::Owned(Sketch::face().to_string().into_bytes()),
        }
    }
}

/// A drawing as an SVG file: filled outlines of pencil strokes in one colour.
#[derive(Debug)]
struct Sketch {
    width: u32,
    height: u32,
    tone: Colour,
    strokes: Paths,
}

impl Sketch {
    /// The hand of the dashboard: an MS Paint pencil, one width.
    fn hand() -> Hand {
        Hand::default()
    }

    /// The width, in pixels, of every pencil line.
    const WEIGHT: f64 = 3.0;

    /// A box in a large square, four lines whose corners cross. The square is inset so the lines
    /// sit inside the slice the stylesheet cuts for the corners.
    fn outline(wobble: Wobble) -> Self {
        let inset = 9.0;
        let far = f64::from(Asset::SQUARE) - inset;
        let around = Rectangle { top_left: Point::new(inset, inset), bottom_right: Point::new(far, far) };
        let mut chance = Mulberry::from(Seed::of("outline").and(wobble.number()));
        let strokes = Edges { around, slack: 6.0 }.draw(&Self::hand(), &Pencil::new(Self::WEIGHT), &mut chance);
        Self { width: Asset::SQUARE, height: Asset::SQUARE, tone: Colour::Ink, strokes }
    }

    /// A box in a small square, so a short side is not squeezed out of a long one: the frame of
    /// a tab, a button or a field.
    fn small() -> Self {
        let inset = 9.0;
        let far = f64::from(Asset::LITTLE) - inset;
        let around = Rectangle { top_left: Point::new(inset, inset), bottom_right: Point::new(far, far) };
        let mut chance = Mulberry::from(Seed::of("outline").and("small"));
        let strokes = Edges { around, slack: 4.0 }.draw(&Self::hand(), &Pencil::new(Self::WEIGHT), &mut chance);
        Self { width: Asset::LITTLE, height: Asset::LITTLE, tone: Colour::Ink, strokes }
    }

    /// A line across a wide strip, to sit under a row.
    fn rule() -> Self {
        let line = Line { from: Point::new(2.0, 7.0), to: Point::new(598.0, 7.0) };
        let strokes = line.draw(&Self::hand(), &Pencil::new(Self::WEIGHT), &mut Mulberry::from(Seed::of("rule")));
        Self { width: 600, height: 14, tone: Colour::Ink, strokes }
    }

    /// The red pen's line under a link.
    fn underline() -> Self {
        let line = Line { from: Point::new(2.0, 4.0), to: Point::new(198.0, 4.0) };
        let strokes = line.draw(&Self::hand(), &Pencil::new(2.4), &mut Mulberry::from(Seed::of("underline")));
        Self { width: 200, height: 8, tone: Colour::Pen, strokes }
    }

    /// A smiling face: a ring, two dots and a smile.
    fn face() -> Self {
        let pencil = Pencil::new(2.4);
        let hand = Self::hand();
        let head = Ellipse {
            centre: Point::new(20.0, 20.0),
            radius: Point::new(14.0, 15.0),
            from: -1.2,
            turns: 1.1,
            count: 40,
        };
        let mut strokes = Drawn(Centreline::from(head)).draw(&hand, &pencil, &mut Mulberry::from(Seed::of("face")));
        let eyes = [Point::new(15.0, 17.0), Point::new(25.0, 16.0)];
        for (n, at) in eyes.into_iter().enumerate() {
            let dot = Line { from: at, to: at + Point::new(1.0, 0.0) };
            strokes.extend(dot.draw(&hand, &Pencil::new(3.4), &mut Mulberry::from(Seed::of("eye").and(n))));
        }
        let smile = Centreline::quad(Point::new(14.0, 24.0), Point::new(20.0, 30.0), Point::new(26.0, 24.0), 12);
        strokes.extend(Drawn(smile).draw(&hand, &pencil, &mut Mulberry::from(Seed::of("smile"))));
        Self { width: 40, height: 40, tone: Colour::Ink, strokes }
    }
}

impl fmt::Display for Sketch {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (width, height) = (self.width, self.height);
        write!(
            out,
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}" preserveAspectRatio="none" shape-rendering="crispEdges">"#
        )?;
        for stroke in &self.strokes {
            write!(out, r#"<path fill="{}" d="{stroke}"/>"#, self.tone.rgb())?;
        }
        out.write_str("</svg>")
    }
}
