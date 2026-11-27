//! `brand/figure.toml` into the logo, mark and favicon, drawn with the freehand pen.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path as FilePath, PathBuf};

use freehand::{Centreline, Ellipse, Hand, Highlighter, Mulberry, Path, Pencil, Point, Seed};
use serde::Deserialize;
use token::Colour;
use unspliced::{Render, html};

use crate::Task;
use crate::error::{Error, Flaw};
use crate::output::{Files, Generator};

/// `cargo xtask brand`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Brand;

impl Brand {
    /// The directory every drawing is written into, as `<name>.svg`.
    const DIR: &'static str = "brand";

    /// The file a drawing named `name` is written to.
    fn file(name: &str) -> PathBuf {
        FilePath::new(Self::DIR).join(format!("{name}.svg"))
    }
}

impl Generator for Brand {
    const TASK: Task = Task::Brand;
    const SOURCE: &'static str = "brand/figure.toml";
    type Source = Figure;

    fn generate(&self, figure: &Figure, files: &mut Files<'_>) -> Result<(), Error> {
        for drawing in &figure.drawing {
            let sheet = Sheet { figure, drawing };
            let whole = sheet.paint(Cut::Whole)?.to_string();
            for name in std::iter::once(&drawing.name).chain(&drawing.copies) {
                files.put(Self::file(name), &whole)?;
            }
            if let Some(open) = &drawing.open {
                files.put(Self::file(open), sheet.paint(Cut::Open(open))?.to_string())?;
            }
        }
        files.claim(Self::DIR, "svg")
    }
}

/// The figure file: named polygons, and the drawings made of them.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Figure {
    polygon: BTreeMap<String, Vec<Xy>>,
    drawing: Vec<Drawing>,
}

impl TryFrom<toml::Table> for Figure {
    type Error = toml::de::Error;

    fn try_from(table: toml::Table) -> Result<Self, toml::de::Error> {
        toml::Value::Table(table).try_into()
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Drawing {
    name: String,
    /// A second file, drawn without the parts marked `head`.
    open: Option<String>,
    /// Other names the same drawing is written under.
    #[serde(default)]
    copies: Vec<String>,
    size: Size,
    hand: Shake,
    /// How far every fill misses its lines.
    shift: Xy,
    place: Place,
    fill: Vec<Fill>,
    stroke: Vec<Stroke>,
}

/// A point of the figure file, `[x, y]`.
#[derive(Debug, Clone, Copy, Deserialize)]
struct Xy([f64; 2]);

impl From<Xy> for Point {
    fn from(Xy([x, y]): Xy) -> Self {
        Self::new(x, y)
    }
}

/// A drawing's size in pixels, `[width, height]`.
#[derive(Debug, Clone, Copy, Deserialize)]
struct Size([u32; 2]);

impl Size {
    /// The SVG `viewBox` of a drawing this size.
    fn view(self) -> String {
        let Self([w, h]) = self;
        format!("0 0 {w} {h}")
    }
}

/// How much the hand shakes.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Shake {
    wobble: f64,
    tremor: f64,
}

impl From<&Shake> for Hand {
    fn from(shake: &Shake) -> Self {
        Self { wobble: shake.wobble, tremor: shake.tremor }
    }
}

/// Where a drawing puts the named polygons: scaled, then moved.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Place {
    offset: Xy,
    scale: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Fill {
    shape: Shape,
    colour: String,
    #[serde(default)]
    head: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stroke {
    line: Shape,
    weight: f64,
    #[serde(default = "Stroke::ink")]
    colour: String,
    #[serde(default)]
    head: bool,
}

impl Stroke {
    /// The colour a stroke is drawn in unless it names one.
    fn ink() -> String {
        Colour::Ink.name().to_owned()
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
enum Shape {
    /// Points as given.
    Points(Vec<Xy>),
    /// A placed straight line that ends exactly at `to`.
    Segment {
        from: Xy,
        to: Xy,
    },
    Quad {
        from: Xy,
        control: Xy,
        to: Xy,
        #[serde(default = "Shape::steps")]
        steps: u32,
    },
    Ellipse(Oval),
    /// An ellipse drawn as two overlapping arcs, the way a hand draws a circle in two goes, so
    /// its middle never fills.
    Ring(Oval),
    /// A named polygon's corners, as one line from the first to the last.
    Chain(String),
    /// A named polygon's corners, as a closed shape.
    Polygon(String),
}

impl Shape {
    /// The steps of a quad unless it says.
    const fn steps() -> u32 {
        24
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Oval {
    centre: Xy,
    radius: Xy,
    #[serde(default = "Oval::start")]
    from: f64,
    #[serde(default = "Oval::turns")]
    turns: f64,
    #[serde(default = "Oval::count")]
    points: u32,
}

impl Oval {
    /// Where an oval starts unless it says, in turns.
    const fn start() -> f64 {
        -0.4
    }

    /// How far round an oval goes unless it says.
    const fn turns() -> f64 {
        1.0
    }

    /// How many points an oval has unless it says.
    const fn count() -> u32 {
        64
    }

    fn line(&self) -> Centreline {
        Centreline::from(Ellipse::from(self))
    }
}

impl From<&Oval> for Ellipse {
    fn from(oval: &Oval) -> Self {
        let Oval { centre, radius, from, turns, points } = *oval;
        Self { centre: centre.into(), radius: radius.into(), from, turns, count: points }
    }
}

/// What is wrong with the figure.
#[derive(Debug)]
pub(crate) enum Problem {
    Polygon { drawing: String, name: String },
    Colour { drawing: String, name: String },
    Ring { drawing: String, points: u32 },
}

impl fmt::Display for Problem {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Polygon { drawing, name } => write!(out, "{drawing}: no polygon `{name}` in [polygon]"),
            Self::Colour { drawing, name } => {
                let unknown = token::Unknown { kind: token::Kind::Colour, name, known: Colour::NAMES };
                write!(out, "{drawing}: {unknown}")
            }
            Self::Ring { drawing, points } => write!(out, "{drawing}: a ring of {points} points is too small to split"),
        }
    }
}

impl From<Problem> for Error {
    fn from(problem: Problem) -> Self {
        Self::Source { path: Brand::SOURCE, flaw: Flaw::Brand(problem) }
    }
}

/// Which parts of a drawing are drawn: all of them, or all but the `head` parts under the
/// drawing's `open` name.
#[derive(Debug, Clone, Copy)]
enum Cut<'a> {
    Whole,
    Open(&'a str),
}

impl Cut<'_> {
    /// Whether a part is drawn, `head` saying whether it is marked so.
    const fn keeps(self, head: bool) -> bool {
        matches!(self, Self::Whole) || !head
    }
}

/// One drawing, ready to resolve shapes against the named polygons.
struct Sheet<'a> {
    figure: &'a Figure,
    drawing: &'a Drawing,
}

impl Sheet<'_> {
    fn polygon(&self, name: &str) -> Result<Centreline, Problem> {
        let corners = self
            .figure
            .polygon
            .get(name)
            .ok_or_else(|| Problem::Polygon { drawing: self.drawing.name.clone(), name: name.to_owned() })?;
        let place = &self.drawing.place;
        Ok(corners.iter().copied().map(Point::from).collect::<Centreline>().placed(place.offset.into(), place.scale))
    }

    /// The lines a shape is drawn as: one, or two for a ring.
    fn lines(&self, shape: &Shape) -> Result<Vec<Centreline>, Problem> {
        Ok(match shape {
            Shape::Points(points) => vec![points.iter().copied().map(Point::from).collect()],
            Shape::Segment { from, to } => vec![Centreline::segment((*from).into(), (*to).into())],
            Shape::Quad { from, control, to, steps } => {
                vec![Centreline::quad((*from).into(), (*control).into(), (*to).into(), *steps)]
            }
            Shape::Ellipse(oval) => vec![oval.line()],
            Shape::Ring(oval) => {
                let line = oval.line();
                let half = (oval.points / 2) as usize;
                let n = oval.points as usize;
                let arcs = half.checked_sub(2).and_then(|start| Some([line.part(0..half + 2)?, line.part(start..n)?]));
                let ring = Problem::Ring { drawing: self.drawing.name.clone(), points: oval.points };
                arcs.ok_or(ring)?.into()
            }
            Shape::Chain(name) => vec![Centreline::chain(self.polygon(name)?.points())],
            Shape::Polygon(name) => vec![self.polygon(name)?],
        })
    }

    fn colour(&self, name: &str) -> Result<Colour, Problem> {
        Colour::named(name).map_err(|_| Problem::Colour { drawing: self.drawing.name.clone(), name: name.to_owned() })
    }

    /// The fills and the ink of the drawing, cut as `cut` says. Its seeds are drawn from the
    /// name of the file it is written to.
    fn paint(&self, cut: Cut<'_>) -> Result<Picture, Problem> {
        let drawing = self.drawing;
        let name = match cut {
            Cut::Whole => drawing.name.as_str(),
            Cut::Open(name) => name,
        };
        let mut fills = Vec::new();
        for fill in drawing.fill.iter().filter(|f| cut.keeps(f.head)) {
            for line in self.lines(&fill.shape)? {
                let mut chance = Mulberry::from(Seed::of(name).and("f").and(fills.len()));
                let corners = line.moved(drawing.shift.into());
                fills.push(Painted {
                    d: Highlighter::default().fill(corners.points(), &mut chance),
                    colour: self.colour(&fill.colour)?,
                });
            }
        }
        let hand = Hand::from(&drawing.hand);
        let mut strokes = Vec::new();
        for stroke in drawing.stroke.iter().filter(|s| cut.keeps(s.head)) {
            for line in self.lines(&stroke.line)? {
                let mut chance = Mulberry::from(Seed::of(name).and(strokes.len()));
                let traced = hand.trace(&line, &mut chance);
                strokes.push(Painted {
                    d: Pencil::new(stroke.weight).outline(&traced),
                    colour: self.colour(&stroke.colour)?,
                });
            }
        }
        Ok(Picture { size: drawing.size, fills, strokes })
    }
}

/// One painted path of a drawing.
struct Painted {
    d: Path,
    colour: Colour,
}

impl Painted {
    /// Each path's outline and colour, as the SVG writes them.
    fn shown(painted: &[Self]) -> Vec<(&str, String)> {
        painted.iter().map(|p| (p.d.as_str(), p.colour.rgb().to_string())).collect()
    }
}

/// A drawing painted: its fills under its ink. Its `Display` is the SVG file.
struct Picture {
    size: Size,
    fills: Vec<Painted>,
    strokes: Vec<Painted>,
}

impl fmt::Display for Picture {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Size([w, h]) = self.size;
        let (fills, strokes) = (Painted::shown(&self.fills), Painted::shown(&self.strokes));
        let page = html! {
            svg xmlns viewBox=(self.size.view()) width=(w) height=(h) {
                @for (d, colour) in &fills { path d=(*d) fill=(colour.as_str()); }
                @for (d, colour) in &strokes { path d=(*d) fill=(colour.as_str()) shape-rendering="crispEdges"; }
            }
        };
        writeln!(out, "{}", page.html())
    }
}
