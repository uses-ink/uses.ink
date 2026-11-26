//! `brand/figure.toml` into the logo, mark and favicon, drawn with the freehand pen.

use std::collections::BTreeMap;
use std::fmt;

use freehand::{Centreline, Hand, Highlighter, Mulberry, Path, Pencil, Point, Seed};
use serde::Deserialize;
use token::Colour;
use unspliced::{Render, html};

use crate::error::Error;
use crate::output::Output;

const SOURCE: &str = "brand/figure.toml";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Figure {
    polygon: BTreeMap<String, Vec<[f64; 2]>>,
    drawing: Vec<Drawing>,
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
    size: [u32; 2],
    hand: HandSpec,
    /// How far every fill misses its lines.
    shift: [f64; 2],
    place: Place,
    fill: Vec<Fill>,
    stroke: Vec<Stroke>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct HandSpec {
    wobble: f64,
    tremor: f64,
}

/// Where a drawing puts the named polygons: scaled, then moved.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Place {
    offset: [f64; 2],
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
    #[serde(default = "ink")]
    colour: String,
    #[serde(default)]
    head: bool,
}

fn ink() -> String {
    Colour::Ink.name().to_owned()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
enum Shape {
    /// Points as given.
    Points(Vec<[f64; 2]>),
    /// A placed straight line that ends exactly at `to`.
    Segment {
        from: [f64; 2],
        to: [f64; 2],
    },
    Quad {
        from: [f64; 2],
        control: [f64; 2],
        to: [f64; 2],
        #[serde(default = "quad_steps")]
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

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Oval {
    centre: [f64; 2],
    radius: [f64; 2],
    #[serde(default = "oval_from")]
    from: f64,
    #[serde(default = "oval_turns")]
    turns: f64,
    #[serde(default = "oval_points")]
    points: u32,
}

fn quad_steps() -> u32 {
    24
}
fn oval_from() -> f64 {
    -0.4
}
fn oval_turns() -> f64 {
    1.0
}
fn oval_points() -> u32 {
    64
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

fn point([x, y]: [f64; 2]) -> Point {
    Point::new(x, y)
}

/// One drawing, ready to resolve shapes against the named polygons.
struct Sheet<'a> {
    figure: &'a Figure,
    drawing: &'a Drawing,
}

impl Sheet<'_> {
    fn polygon(&self, name: &str) -> Result<Centreline, Error> {
        let corners = self.figure.polygon.get(name).ok_or_else(|| {
            Error::Brand(Problem::Polygon { drawing: self.drawing.name.clone(), name: name.to_owned() })
        })?;
        let place = &self.drawing.place;
        Ok(Centreline::through(corners.iter().copied().map(point)).placed(point(place.offset), place.scale))
    }

    /// The lines a shape is drawn as: one, or two for a ring.
    fn lines(&self, shape: &Shape) -> Result<Vec<Centreline>, Error> {
        Ok(match shape {
            Shape::Points(points) => vec![Centreline::through(points.iter().copied().map(point))],
            Shape::Segment { from, to } => vec![Centreline::segment(point(*from), point(*to))],
            Shape::Quad { from, control, to, steps } => {
                vec![Centreline::quad(point(*from), point(*control), point(*to), *steps)]
            }
            Shape::Ellipse(oval) => vec![oval.line()],
            Shape::Ring(oval) => {
                let line = oval.line();
                let half = (oval.points / 2) as usize;
                let n = oval.points as usize;
                let arcs = half.checked_sub(2).and_then(|start| Some([line.part(0..half + 2)?, line.part(start..n)?]));
                let ring = Problem::Ring { drawing: self.drawing.name.clone(), points: oval.points };
                arcs.ok_or(Error::Brand(ring))?.into()
            }
            Shape::Chain(name) => vec![Centreline::chain(self.polygon(name)?.points())],
            Shape::Polygon(name) => vec![self.polygon(name)?],
        })
    }

    fn colour(&self, name: &str) -> Result<Colour, Error> {
        Colour::named(name)
            .map_err(|_| Error::Brand(Problem::Colour { drawing: self.drawing.name.clone(), name: name.to_owned() }))
    }
}

impl Oval {
    fn line(&self) -> Centreline {
        Centreline::ellipse(point(self.centre), point(self.radius), self.from, self.turns, self.points)
    }
}

/// One painted path of a drawing.
struct Painted {
    d: Path,
    colour: Colour,
}

/// The fills and the ink of a drawing, open or whole.
fn paint(sheet: &Sheet<'_>, name: &str, open: bool) -> Result<(Vec<Painted>, Vec<Painted>), Error> {
    let drawing = sheet.drawing;
    let mut fills = Vec::new();
    for fill in drawing.fill.iter().filter(|f| !(open && f.head)) {
        for line in sheet.lines(&fill.shape)? {
            let mut chance = Mulberry::new(Seed::of(name).and("f").and_number(fills.len()));
            let corners = line.moved(point(drawing.shift));
            fills.push(Painted {
                d: Highlighter::default().fill(corners.points(), &mut chance),
                colour: sheet.colour(&fill.colour)?,
            });
        }
    }
    let hand = Hand { wobble: drawing.hand.wobble, tremor: drawing.hand.tremor };
    let mut strokes = Vec::new();
    for stroke in drawing.stroke.iter().filter(|s| !(open && s.head)) {
        for line in sheet.lines(&stroke.line)? {
            let mut chance = Mulberry::new(Seed::of(name).and_number(strokes.len()));
            let traced = hand.trace(&line, &mut chance);
            strokes.push(Painted {
                d: Pencil::new(stroke.weight).outline(&traced),
                colour: sheet.colour(&stroke.colour)?,
            });
        }
    }
    Ok((fills, strokes))
}

/// The SVG file of a drawing.
fn svg(drawing: &Drawing, fills: &[Painted], strokes: &[Painted]) -> String {
    let [w, h] = drawing.size;
    let view = format!("0 0 {w} {h}");
    let fills: Vec<(&str, String)> = fills.iter().map(|p| (p.d.as_str(), p.colour.rgb().to_string())).collect();
    let strokes: Vec<(&str, String)> = strokes.iter().map(|p| (p.d.as_str(), p.colour.rgb().to_string())).collect();
    let page = html! {
        svg xmlns viewBox=(view) width=(w) height=(h) {
            @for (d, colour) in &fills { path d=(*d) fill=(colour.as_str()); }
            @for (d, colour) in &strokes { path d=(*d) fill=(colour.as_str()) shape-rendering="crispEdges"; }
        }
    };
    let mut text = page.html();
    text.push('\n');
    text
}

pub(crate) fn write(output: &mut Output) -> Result<(), Error> {
    let text = output.read(SOURCE)?;
    let figure: Figure = toml::from_str(&text).map_err(|source| Error::Toml { path: SOURCE.into(), source })?;
    for drawing in &figure.drawing {
        let sheet = Sheet { figure: &figure, drawing };
        let (fills, strokes) = paint(&sheet, &drawing.name, false)?;
        let whole = svg(drawing, &fills, &strokes);
        for name in std::iter::once(&drawing.name).chain(&drawing.copies) {
            output.put(format!("brand/{name}.svg"), whole.as_bytes())?;
        }
        if let Some(open) = &drawing.open {
            let (fills, strokes) = paint(&sheet, open, true)?;
            output.put(format!("brand/{open}.svg"), svg(drawing, &fills, &strokes).as_bytes())?;
        }
    }
    output.claim("brand", "svg")
}
