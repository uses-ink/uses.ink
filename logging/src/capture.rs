//! A test subscriber that keeps every span and event as typed data, so a test asserts that the
//! instrumentation exists without reading text. No timestamps and no threads of its own: what
//! it records is the same on every run.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use tracing::span::{Attributes, Id, Record};
use tracing::subscriber::Interest;
use tracing::{Event, Level, Metadata, Subscriber};
use tracing_subscriber::Registry;
use tracing_subscriber::layer::{Context, Layer, SubscriberExt as _};
use tracing_subscriber::registry::LookupSpan;

use crate::record::{Fields, Value};

/// Whether a [`Seen`] is a span or an event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Shape {
    /// A span, recorded when it opened; fields recorded later are merged in.
    Span,
    /// An event.
    Event,
}

/// One span or event.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct Seen {
    /// Span or event.
    pub shape: Shape,
    /// Its name: the constant of [`name`](crate::name).
    pub name: &'static str,
    /// Its level.
    pub level: Level,
    /// Its target: the module that wrote it.
    pub target: &'static str,
    /// Its typed fields.
    pub fields: Fields,
    /// The names of the spans it sits in, outermost first.
    pub within: Vec<&'static str>,
    /// For a span, whether it has closed.
    pub closed: bool,
}

impl Seen {
    /// The value of a field.
    pub fn field(&self, key: &str) -> Option<&Value> {
        self.fields.get(key)
    }

    /// A numeric field.
    pub fn number(&self, key: &str) -> Option<u64> {
        self.field(key).and_then(Value::unsigned)
    }

    /// A text field.
    pub fn text(&self, key: &str) -> Option<&str> {
        self.field(key).and_then(Value::text)
    }

    /// The `kind` field, which tells apart spans of one name.
    pub fn kind(&self) -> Option<&str> {
        self.text("kind")
    }
}

/// The records of one run.
#[derive(Clone, Debug)]
pub struct Capture {
    level: Level,
    seen: Arc<Mutex<Vec<Seen>>>,
}

/// The index of a span's record, kept in the span.
#[derive(Clone, Copy, Debug)]
struct Slot(usize);

impl Default for Capture {
    fn default() -> Self {
        Self::at(Level::TRACE)
    }
}

impl Capture {
    /// A capture of everything.
    pub fn new() -> Self {
        Self::default()
    }

    /// A capture of `level` and the more severe levels: what a subscriber set to `level` sees.
    pub fn at(level: Level) -> Self {
        Self { level, seen: Arc::default() }
    }

    fn lock(&self) -> MutexGuard<'_, Vec<Seen>> {
        self.seen.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Runs `work` with this capture as the subscriber of the current thread, and returns what
    /// it returned. A test that drives a future on this thread (`pollster`, a current-thread
    /// runtime) is covered.
    pub fn run<T>(&self, work: impl FnOnce() -> T) -> T {
        let subscriber = Registry::default().with(self.clone());
        tracing::subscriber::with_default(subscriber, work)
    }

    /// Everything seen, in the order it happened (a span at its opening).
    pub fn all(&self) -> Vec<Seen> {
        self.lock().clone()
    }

    /// The events, in order.
    pub fn events(&self) -> Vec<Seen> {
        self.lock().iter().filter(|seen| seen.shape == Shape::Event).cloned().collect()
    }

    /// The spans, in the order they opened.
    pub fn spans(&self) -> Vec<Seen> {
        self.lock().iter().filter(|seen| seen.shape == Shape::Span).cloned().collect()
    }

    /// The events and spans of one name.
    pub fn named(&self, name: &str) -> Vec<Seen> {
        self.lock().iter().filter(|seen| seen.name == name).cloned().collect()
    }

    /// The `kind` of every span of one name, in order: the stages of a build, say.
    pub fn kinds(&self, span: &str) -> Vec<String> {
        self.spans().iter().filter(|seen| seen.name == span).filter_map(|seen| seen.kind().map(str::to_owned)).collect()
    }
}

impl<S> Layer<S> for Capture
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn register_callsite(&self, _: &'static Metadata<'static>) -> Interest {
        // decided per call in `enabled`: a callsite is registered once, for every test
        Interest::sometimes()
    }

    fn enabled(&self, metadata: &Metadata<'_>, _: Context<'_, S>) -> bool {
        *metadata.level() <= self.level
    }

    fn on_new_span(&self, attributes: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let mut fields = Fields::default();
        attributes.record(&mut fields);
        let within = ctx.span_scope(id).map(|scope| scope.skip(1).map(|span| span.name()).collect::<Vec<_>>());
        let mut within = within.unwrap_or_default();
        within.reverse();
        let mut seen = self.lock();
        let metadata = attributes.metadata();
        seen.push(Seen {
            shape: Shape::Span,
            name: metadata.name(),
            level: *metadata.level(),
            target: metadata.target(),
            fields,
            within,
            closed: false,
        });
        let slot = Slot(seen.len() - 1);
        drop(seen);
        if let Some(span) = ctx.span(id) {
            span.extensions_mut().insert(slot);
        }
    }

    fn on_record(&self, id: &Id, values: &Record<'_>, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(id) else { return };
        let Some(Slot(at)) = span.extensions().get::<Slot>().copied() else { return };
        let mut seen = self.lock();
        if let Some(entry) = seen.get_mut(at) {
            values.record(&mut entry.fields);
        }
    }

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let mut fields = Fields::default();
        event.record(&mut fields);
        let mut within: Vec<&'static str> =
            ctx.event_scope(event).map_or_default(|scope| scope.map(|span| span.name()).collect());
        within.reverse();
        let metadata = event.metadata();
        self.lock().push(Seen {
            shape: Shape::Event,
            name: metadata.name(),
            level: *metadata.level(),
            target: metadata.target(),
            fields,
            within,
            closed: false,
        });
    }

    fn on_close(&self, id: Id, ctx: Context<'_, S>) {
        let Some(span) = ctx.span(&id) else { return };
        let Some(Slot(at)) = span.extensions().get::<Slot>().copied() else { return };
        if let Some(entry) = self.lock().get_mut(at) {
            entry.closed = true;
        }
    }
}
