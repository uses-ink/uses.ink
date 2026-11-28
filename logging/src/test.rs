//! What the crate promises, checked with its own `Capture`.

use std::cell::Cell;
use std::error::Error;
use std::fmt;

use crate::{Capture, Count, Failure, Field, Level, Redacted, Secret, Shown, Size, Stage, Unit, Value, event};

#[derive(Debug)]
struct Inner;

impl fmt::Display for Inner {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("the disk said no")
    }
}

impl Error for Inner {}

#[derive(Debug)]
struct Outer(Inner);

impl fmt::Display for Outer {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("the blob could not be stored")
    }
}

impl Error for Outer {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.0)
    }
}

#[test]
fn spans_nest_and_events_carry_the_spans_they_sit_in() {
    let capture = Capture::new();
    capture.run(|| {
        let _build = crate::build!(7_u64, "folder").entered();
        for stage in Stage::ALL {
            let _stage = crate::stage!(stage, "test").entered();
            let _unit = crate::unit!(Unit::Page, "posts/a.md").entered();
            crate::debug!(name: event::UNIT_PLANNED, bytes = Size(5).field());
        }
    });
    assert_eq!(capture.kinds("stage"), ["fetch", "discover", "source", "decide", "image", "render", "write"]);
    let planned = capture.named(event::UNIT_PLANNED);
    assert_eq!(planned.len(), 7);
    assert_eq!(planned[0].within, ["build", "stage", "unit"]);
    assert_eq!(planned[0].number("bytes"), Some(5));
    assert_eq!(capture.spans()[0].text("build"), Some("7"));
    assert!(capture.spans().iter().all(|span| span.closed));
}

#[test]
fn a_field_recorded_after_the_span_opened_is_kept() {
    let capture = Capture::new();
    capture.run(|| {
        let span = crate::request!(1, "GET", "/a");
        span.record("status", Count(404).field());
    });
    assert_eq!(capture.named("request")[0].number("status"), Some(404));
}

#[test]
fn an_error_keeps_its_causes_as_data_not_as_one_string() {
    let capture = Capture::new();
    capture.run(|| {
        let error = Outer(Inner);
        crate::error!(name: event::JOB_FAILED, error = Failure(&error).field());
    });
    let chain = capture.events()[0].field("error").and_then(Value::chain).cloned().unwrap();
    assert_eq!(chain.message, "the blob could not be stored");
    assert_eq!(chain.causes, ["the disk said no"]);
}

#[test]
fn a_secret_records_as_redacted_whatever_it_holds() {
    let capture = Capture::new();
    capture.run(|| {
        crate::info!(name: event::API_AUTH, token = Secret("hunter2").field(), also = %Secret(vec![1_u8]), shown = ?Secret(1_u8));
    });
    let seen = &capture.events()[0];
    for key in ["token", "also", "shown"] {
        assert_eq!(seen.text(key), Some(Redacted::TEXT), "{key}");
    }
}

#[derive(Default)]
struct Probe(Cell<u32>);

impl Probe {
    fn field(&self) -> u64 {
        self.0.set(self.0.get() + 1);
        1
    }
}

#[test]
fn a_disabled_level_converts_no_field() {
    let probe = Probe::default();
    let capture = Capture::at(Level::INFO);
    capture.run(|| {
        for _ in 0..1000 {
            crate::trace!(name: event::UNIT_PLANNED, n = probe.field());
            crate::debug!(name: event::UNIT_PLANNED, n = probe.field());
        }
        crate::info!(name: event::UNIT_PLANNED, n = probe.field());
    });
    // the conversions are arguments of the macro: only the enabled event ran one
    assert_eq!(probe.0.get(), 1);
    assert_eq!(capture.events().len(), 1);
}

#[test]
fn a_lazy_display_is_not_formatted_when_disabled() {
    struct Loud<'a>(&'a Cell<u32>);
    impl fmt::Display for Loud<'_> {
        fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
            self.0.set(self.0.get() + 1);
            out.write_str("x")
        }
    }
    let formatted = Cell::new(0);
    Capture::at(Level::ERROR).run(|| crate::info!(name: event::UNIT_PLANNED, id = Shown(Loud(&formatted)).field()));
    assert_eq!(formatted.get(), 0);
}
