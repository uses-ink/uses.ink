//! The host's capabilities, implemented once and linked into every component (architecture.md
//! section 6): the pen through `freehand`, text measuring, highlighting through host-native's
//! [`Grammars`], and the record of what a plugin says. Each call's store holds a [`State`].

use engine::{Record, Said};
use freehand::mark::{Drawn, Fill, Frame, Line, Patch, Ring, Underline};
use freehand::{Chance, Hand, Highlighter, Mark, Mulberry, Paths, Pencil, Point, Seed};
use host_native::Grammars;
use model::highlight::Highlight;
use model::{Diagnostic, Notice, Problem};
use wasmtime::ResourceLimiter;

use crate::bind::plugin::ink::host::{diagnostic, highlight, pen, text};
use crate::bind::plugin::ink::model as types;
use crate::budget::Budget;

/// What one store holds: the plugin's name, the grammars, what the plugin said, and its memory
/// limit.
#[derive(Debug)]
pub(crate) struct State {
    plugin: String,
    grammars: Grammars,
    pub(crate) said: Said,
    pub(crate) limit: Limit,
}

impl State {
    pub(crate) fn new(plugin: impl Into<String>, grammars: Grammars, budget: &Budget) -> Self {
        let limit = Limit { memory: budget.bytes(), hit: false };
        Self { plugin: plugin.into(), grammars, said: Said::default(), limit }
    }

    /// `plugin.value.malformed`: the plugin sent something the model refuses.
    fn malformed(&mut self, refused: &impl std::error::Error) {
        let problem = Problem::PluginValueMalformed { plugin: self.plugin.clone(), reason: refused.to_string() };
        self.said.diagnose(Diagnostic::new(problem));
    }
}

/// The memory an instance may grow to, and whether it asked for more.
#[derive(Debug)]
pub(crate) struct Limit {
    memory: usize,
    pub(crate) hit: bool,
}

impl ResourceLimiter for Limit {
    fn memory_growing(&mut self, _current: usize, desired: usize, _maximum: Option<usize>) -> wasmtime::Result<bool> {
        let allowed = desired <= self.memory;
        self.hit |= !allowed;
        Ok(allowed)
    }

    fn table_growing(&mut self, _current: usize, desired: usize, _maximum: Option<usize>) -> wasmtime::Result<bool> {
        Ok(desired <= 1 << 16)
    }
}

/// A mark drawn with whichever tool was asked for.
struct Drawing<'m, M>(&'m M);

impl<M: Mark> Drawing<'_, M> {
    fn with(&self, tool: pen::Tool, hand: &Hand, chance: &mut impl Chance) -> Paths {
        match tool {
            pen::Tool::Pencil(weight) => self.0.draw(hand, &Pencil::new(weight), chance),
            pen::Tool::Highlighter(h) => {
                self.0.draw(hand, &Highlighter { spacing: h.spacing, jitter: h.jitter }, chance)
            }
        }
    }
}

impl pen::Host for State {
    fn draw(&mut self, mark: pen::Mark, tool: pen::Tool, hand: pen::Hand, seed: String) -> Vec<String> {
        let hand = Hand::from(hand);
        let mut chance = Mulberry::from(Seed::of(&seed));
        let points = |points: Vec<pen::Point>| points.into_iter().map(Point::from).collect::<Vec<_>>();
        let paths = match mark {
            pen::Mark::Line(line) => {
                Drawing(&Line { from: line.start.into(), to: line.end.into() }).with(tool, &hand, &mut chance)
            }
            pen::Mark::Underline(u) => Drawing(&Underline { width: u.width, y: u.y }).with(tool, &hand, &mut chance),
            pen::Mark::Frame(f) => {
                Drawing(&Frame { around: f.around.into(), slack: f.slack }).with(tool, &hand, &mut chance)
            }
            pen::Mark::Ring(ellipse) => Drawing(&Ring(ellipse.into())).with(tool, &hand, &mut chance),
            pen::Mark::Fill(f) => {
                Drawing(&Fill { behind: f.behind.into(), shift: f.shift.into() }).with(tool, &hand, &mut chance)
            }
            pen::Mark::Patch(corners) => Drawing(&Patch(points(corners))).with(tool, &hand, &mut chance),
            pen::Mark::Drawn(line) => Drawing(&Drawn(points(line).into_iter().collect::<freehand::Centreline>())).with(
                tool,
                &hand,
                &mut chance,
            ),
        };
        paths.iter().map(|path| path.as_str().to_owned()).collect()
    }
}

/// Measuring text needs fonts, and the host has none yet (the measurer of architecture.md
/// section 4 is not built): every font is unknown, said as the typed refusal.
impl text::Host for State {
    fn measure(&mut self, _font: String, _size: f64, _text: String) -> Result<text::Extent, text::Unmeasured> {
        Err(text::Unmeasured::FontUnknown)
    }
}

impl highlight::Host for State {
    fn highlight(&mut self, language: String, code: String) -> Result<Vec<highlight::Run>, highlight::Unhighlighted> {
        let runs =
            self.grammars.highlight(&language, &code).map_err(|refused| highlight::Unhighlighted::from(&refused))?;
        runs.into_iter().map(highlight::Run::try_from).collect::<Result<_, _>>().map_err(|unmirrored| {
            let problem = Problem::PluginValueUnmirrored { reason: unmirrored.to_string() };
            self.said.diagnose(Diagnostic::new(problem));
            highlight::Unhighlighted::Failed
        })
    }
}

impl diagnostic::Host for State {
    fn emit(&mut self, diagnostic: types::diagnostic::Diagnostic) {
        match Diagnostic::try_from(diagnostic) {
            Ok(diagnostic) => self.said.diagnose(diagnostic),
            Err(refused) => self.malformed(&refused),
        }
    }

    fn announce(&mut self, notice: types::diagnostic::Notice) {
        match Notice::try_from(notice) {
            Ok(notice) => self.said.notice(notice),
            Err(refused) => self.malformed(&refused),
        }
    }
}

impl types::diagnostic::Host for State {}
impl types::document::Host for State {}
impl types::site::Host for State {}
impl types::read::Host for State {}
impl types::file::Host for State {}
