//! Progress: a stage with a count of units, drawn as one live line on a terminal, as a heartbeat
//! line in a log, and as `uses.progress/v1` events with `--json` (`docs/cli.md` section 8).
//!
//! The meter decides when something is drawn and returns what to draw; it writes nothing and reads
//! time only through a [`Clock`], so a test moves time by hand.

use std::fmt;
use std::time::Duration;

use clock::Clock;
use jiff::Timestamp;
use serde::Serialize;

use super::doc::{Doc, Line, Render};
use super::glyph::Glyph;
use super::layout::{Columns, Cut, Flex, Gutter};
use super::role::{Role, Strength};

/// The stages of a build, shared by the live line, the log and the event.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Phase {
    Reading,
    Understanding,
    Rendering,
    Writing,
    Checking,
    Uploading,
    Building,
}

impl Phase {
    /// The word a stage in progress starts with.
    pub(crate) const fn doing(self) -> &'static str {
        match self {
            Self::Reading => "Reading",
            Self::Understanding => "Understanding",
            Self::Rendering => "Rendering",
            Self::Writing => "Writing",
            Self::Checking => "Checking",
            Self::Uploading => "Uploading",
            Self::Building => "Building",
        }
    }
}

/// How long something took, as a person reads it: `1.4 s`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Elapsed(pub(crate) Duration);

impl fmt::Display for Elapsed {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{:.1} s", self.0.as_secs_f64())
    }
}

/// Whether a stage is still going or has ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Moment {
    Running,
    Finished,
}

/// One progress event, `uses.progress/v1`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct Beat {
    schema: &'static str,
    stage: Phase,
    state: Moment,
    #[serde(skip_serializing_if = "Option::is_none")]
    done: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    total: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    item: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
}

impl Beat {
    /// The schema of every event.
    pub(crate) const SCHEMA: &'static str = "uses.progress/v1";

    /// A stage that ended, with the words its finished line says.
    pub(crate) fn finished(stage: Phase, detail: String) -> Self {
        Self {
            schema: Self::SCHEMA,
            stage,
            state: Moment::Finished,
            done: None,
            total: None,
            item: None,
            detail: Some(detail),
        }
    }

    /// A stage that is going, at `tick`.
    fn running(tick: &Tick) -> Self {
        Self {
            schema: Self::SCHEMA,
            stage: tick.phase,
            state: Moment::Running,
            done: Some(tick.done),
            total: Some(tick.total),
            item: tick.item.clone(),
            detail: None,
        }
    }
}

/// How often each kind of progress is drawn. Every value is the design's and is a setting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Pacing {
    /// How long a stage works before a live line appears, so a fast build does not flicker.
    pub(crate) appear: Duration,
    /// How often a live line is redrawn.
    pub(crate) refresh: Duration,
    /// How often a log line says a stage is still going.
    pub(crate) heartbeat: Duration,
    /// How often an event is written.
    pub(crate) events: Duration,
    /// Cells in the bar.
    pub(crate) bar: usize,
}

impl Default for Pacing {
    fn default() -> Self {
        Self {
            appear: Duration::from_millis(500),
            refresh: Duration::from_millis(100),
            heartbeat: Duration::from_secs(5),
            events: Duration::from_millis(500),
            bar: 24,
        }
    }
}

/// How progress is shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Show {
    /// One line, redrawn.
    Live,
    /// A heartbeat line when a stage is long.
    Log,
    /// Events on stderr.
    Events,
    /// Not at all.
    Off,
}

/// A stage at one moment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Tick {
    pub(crate) phase: Phase,
    pub(crate) done: u64,
    pub(crate) total: u64,
    pub(crate) unit: &'static str,
    pub(crate) item: Option<String>,
    pub(crate) elapsed: Duration,
    pub(crate) bar: usize,
}

impl Tick {
    /// The bar: `━━━━╸────`, filled in proportion to what is done.
    fn bar(&self, doc: &Doc) -> Line {
        let cells = self.bar;
        let filled = if self.total == 0 {
            cells
        } else {
            usize::try_from(u128::from(self.done.min(self.total)) * cells as u128 / u128::from(self.total))
                .unwrap_or(cells)
        };
        let glyph = |glyph| doc.look().glyphs.get(glyph);
        let role = Glyph::BarFull.role();
        if filled >= cells {
            return Line::new().with(role, glyph(Glyph::BarFull).repeat(cells));
        }
        Line::new()
            .with(role, glyph(Glyph::BarFull).repeat(filled))
            .with(role, glyph(Glyph::BarHead))
            .with(role, glyph(Glyph::BarRest).repeat(cells - filled - 1))
    }

    /// The stage as the live line draws it.
    pub(crate) const fn live(&self) -> Live<'_> {
        Live(self)
    }

    /// The stage as a heartbeat line says it.
    pub(crate) const fn heartbeat(&self) -> Heartbeat<'_> {
        Heartbeat(self)
    }
}

/// The live line: the bar, the count and the current item.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Live<'a>(&'a Tick);

impl Render for Live<'_> {
    fn render(&self, into: &mut Doc) {
        let tick = self.0;
        let counts = Line::new().with(Role::Plain, format!("{}/{} {}", tick.done, tick.total, tick.unit));
        let item = Line::new().with(Role::Quiet, tick.item.as_deref().unwrap_or_default());
        let rows = [vec![tick.bar(into), counts, item]];
        let lead = Gutter::MARGIN + Gutter::WIDTH + 2;
        let flex = Some(Flex { column: 2, cut: Cut::Middle });
        let text = Columns { rows: &rows, flex, lead }.lines(into).into_iter().next().unwrap_or_default();
        Gutter { word: tick.phase.doing().to_owned(), role: Role::Work, strength: Strength::Normal, text }.render(into);
    }
}

/// A line of a log that says a stage is still going.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Heartbeat<'a>(&'a Tick);

impl Render for Heartbeat<'_> {
    fn render(&self, into: &mut Doc) {
        let tick = self.0;
        let text = format!("{} {}, {} done, {}", tick.total, tick.unit, tick.done, Elapsed(tick.elapsed));
        Gutter {
            word: tick.phase.doing().to_owned(),
            role: Role::Work,
            strength: Strength::Normal,
            text: Line::plain(text),
        }
        .render(into);
    }
}

/// What the meter asks to be drawn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Draw {
    /// Replace the live line.
    Redraw(Tick),
    /// Print a log line.
    Heartbeat(Tick),
    /// Write an event.
    Event(Beat),
    /// Nothing yet.
    Nothing,
}

/// A stage that is running.
#[derive(Clone, Copy, Debug)]
struct Running {
    phase: Phase,
    total: u64,
    unit: &'static str,
    began: Timestamp,
    last: Option<Timestamp>,
}

/// Decides when progress is drawn.
#[derive(Debug)]
pub(crate) struct Meter<C> {
    clock: C,
    pacing: Pacing,
    show: Show,
    running: Option<Running>,
    drawn: bool,
    lap: Timestamp,
}

impl<C: Clock> Meter<C> {
    /// A meter that shows progress as `show` says, paced by `pacing`.
    pub(crate) fn new(clock: C, pacing: Pacing, show: Show) -> Self {
        let lap = clock.now();
        Self { clock, pacing, show, running: None, drawn: false, lap }
    }

    /// A stage of `total` units begins.
    pub(crate) fn begin(&mut self, phase: Phase, total: u64, unit: &'static str) {
        let began = self.clock.now();
        self.running = Some(Running { phase, total, unit, began, last: None });
    }

    /// `done` units are finished, `item` the latest started. Returns what to draw.
    pub(crate) fn tick(&mut self, done: u64, item: Option<&str>) -> Draw {
        let now = self.clock.now();
        let (pacing, show) = (self.pacing, self.show);
        let Some(running) = self.running.as_mut() else { return Draw::Nothing };
        let tick = Tick {
            phase: running.phase,
            done,
            total: running.total,
            unit: running.unit,
            item: item.map(ToOwned::to_owned),
            elapsed: Self::gap(now, running.began),
            bar: pacing.bar,
        };
        let since = running.last.map(|last| Self::gap(now, last));
        let due = |every: Duration| since.is_none_or(|since| since >= every);
        let draw = match show {
            Show::Live if tick.elapsed >= pacing.appear && due(pacing.refresh) => Draw::Redraw(tick),
            Show::Log if since.unwrap_or(tick.elapsed) >= pacing.heartbeat => Draw::Heartbeat(tick),
            Show::Events if due(pacing.events) => Draw::Event(Beat::running(&tick)),
            Show::Off | Show::Live | Show::Log | Show::Events => Draw::Nothing,
        };
        if draw != Draw::Nothing {
            running.last = Some(now);
        }
        if matches!(draw, Draw::Redraw(_)) {
            self.drawn = true;
        }
        draw
    }

    /// The stage ends. Whether a live line is on the screen and must be erased.
    pub(crate) fn end(&mut self) -> bool {
        self.running = None;
        std::mem::take(&mut self.drawn)
    }

    /// Time since the last lap, or since the meter began; starts a new lap.
    pub(crate) fn lap(&mut self) -> Duration {
        let now = self.clock.now();
        Self::gap(now, std::mem::replace(&mut self.lap, now))
    }

    fn gap(now: Timestamp, then: Timestamp) -> Duration {
        now.duration_since(then).unsigned_abs()
    }
}
