//! The one layer behind `Logging`: it keeps what every span needs, then hands each span and
//! event, with the spans around it, to the sinks that were asked for.

use std::io::Write;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Level, Subscriber};
use tracing_subscriber::layer::{Context, Layer};
use tracing_subscriber::registry::{LookupSpan, SpanRef};

use crate::human::Human;
use crate::lines::Lines;
use crate::record::Fields;
use crate::timing::Timing;

/// Where text goes. A writer that fails has nowhere to say so, because it is the log.
#[derive(Clone)]
pub(crate) struct Sink(Arc<Mutex<Box<dyn Write + Send>>>);

impl std::fmt::Debug for Sink {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str("Sink")
    }
}

impl Sink {
    pub(crate) fn new(writer: impl Write + Send + 'static) -> Self {
        Self(Arc::new(Mutex::new(Box::new(writer))))
    }

    pub(crate) fn put(&self, text: &str) {
        let mut writer = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        // the log has no channel to report its own failure on
        writer.write_all(text.as_bytes()).ok();
    }
}

/// What a span keeps while it is open.
#[derive(Debug)]
struct Open {
    at: Instant,
    fields: Fields,
    children: Duration,
}

/// A span around an entry, as it was when the entry was written.
#[derive(Clone, Debug)]
pub(crate) struct Frame {
    pub(crate) name: &'static str,
    pub(crate) fields: Fields,
}

impl Frame {
    fn of<S: for<'a> LookupSpan<'a>>(span: &SpanRef<'_, S>) -> Self {
        let fields = span.extensions().get::<Open>().map_or_default(|open| open.fields.clone());
        Self { name: span.name(), fields }
    }

    /// `name`, or `name[kind]` when the span has one.
    pub(crate) fn label(&self) -> String {
        match self.fields.get("kind").and_then(|value| value.text()) {
            Some(kind) => format!("{}[{kind}]", self.name),
            None => self.name.to_owned(),
        }
    }
}

/// What an entry is.
#[derive(Debug)]
pub(crate) enum What<'a> {
    Open,
    Close { elapsed: Duration, own: Duration },
    Event { name: &'a str },
}

/// One line's worth: when, how severe, who wrote it, what it says and where it sits.
#[derive(Debug)]
pub(crate) struct Entry<'a> {
    pub(crate) at: Duration,
    pub(crate) level: Level,
    pub(crate) target: &'a str,
    pub(crate) what: What<'a>,
    /// The spans around it, outermost first; for a span, its own frame is last.
    pub(crate) frames: &'a [Frame],
    pub(crate) fields: &'a Fields,
}

/// The layer.
#[derive(Debug)]
pub(crate) struct Logger {
    pub(crate) start: Instant,
    pub(crate) human: Option<Human>,
    pub(crate) lines: Vec<Lines>,
    pub(crate) timing: Option<Timing>,
}

impl Logger {
    fn say(&self, entry: &Entry<'_>) {
        if let Some(human) = &self.human {
            human.write(entry);
        }
        for lines in &self.lines {
            lines.write(entry);
        }
    }

    fn chain<S: for<'a> LookupSpan<'a>>(span: &SpanRef<'_, S>) -> Vec<Frame> {
        span.scope().from_root().map(|up| Frame::of(&up)).collect()
    }
}

impl<S> Layer<S> for Logger
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(&self, attributes: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(id) else { return };
        let mut fields = Fields::default();
        attributes.record(&mut fields);
        span.extensions_mut().insert(Open { at: Instant::now(), fields, children: Duration::ZERO });
        let frames = Self::chain(&span);
        if let Some(timing) = &self.timing {
            timing.open(&frames.iter().map(Frame::label).collect::<Vec<_>>());
        }
        let metadata = attributes.metadata();
        let own = frames.last().map_or_default(|frame| frame.fields.clone());
        self.say(&Entry {
            at: self.start.elapsed(),
            level: *metadata.level(),
            target: metadata.target(),
            what: What::Open,
            frames: &frames,
            fields: &own,
        });
    }

    fn on_record(&self, id: &Id, values: &Record<'_>, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(id) else { return };
        if let Some(open) = span.extensions_mut().get_mut::<Open>() {
            values.record(&mut open.fields);
        }
    }

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let mut fields = Fields::default();
        event.record(&mut fields);
        let frames: Vec<Frame> =
            ctx.event_scope(event).map_or_default(|scope| scope.from_root().map(|span| Frame::of(&span)).collect());
        let metadata = event.metadata();
        self.say(&Entry {
            at: self.start.elapsed(),
            level: *metadata.level(),
            target: metadata.target(),
            what: What::Event { name: metadata.name() },
            frames: &frames,
            fields: &fields,
        });
    }

    fn on_close(&self, id: Id, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(&id) else { return };
        let Some((elapsed, own, fields)) = span.extensions().get::<Open>().map(|open| {
            let elapsed = open.at.elapsed();
            (elapsed, elapsed.saturating_sub(open.children), open.fields.clone())
        }) else {
            return;
        };
        if let Some(parent) = span.parent().as_ref()
            && let Some(open) = parent.extensions_mut().get_mut::<Open>()
        {
            open.children += elapsed;
        }
        let frames = Self::chain(&span);
        if let Some(timing) = &self.timing {
            timing.close(&frames.iter().map(Frame::label).collect::<Vec<_>>(), elapsed, own);
        }
        self.say(&Entry {
            at: self.start.elapsed(),
            level: *span.metadata().level(),
            target: span.metadata().target(),
            what: What::Close { elapsed, own },
            frames: &frames,
            fields: &fields,
        });
    }
}
