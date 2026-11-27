//! The layers of the span tree, outermost to innermost, and one macro for each.
//!
//! A macro, not a function, so the span's target is the module that opened it: an area filter
//! (`INK_LOG=edge=trace`) then governs the structure around an area's events as well as the
//! events. Every macro takes typed values and calls [`Field::field`](crate::Field::field) on
//! them inside the `tracing` macro, so nothing runs when the level is off. Fields declared
//! `Empty` are recorded later with `Span::record`.

use crate::Kind;

/// The stages of a build, in the order a build runs them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Stage {
    /// Bringing the snapshot from its source: the git fetch, the folder read.
    Fetch,
    /// Finding what the snapshot holds: scope, layout, ignores, the vault's settings.
    Discover,
    /// Reading each kept file through its format.
    Source,
    /// Decisions: pages, links, assets, routes, the assembled site.
    Decide,
    /// Preparing the raster images.
    Image,
    /// Rendering the artifacts.
    Render,
    /// Storing the blobs and recording the deploy.
    Write,
}

impl Stage {
    /// Every stage, in order.
    pub const ALL: [Self; 7] =
        [Self::Fetch, Self::Discover, Self::Source, Self::Decide, Self::Image, Self::Render, Self::Write];
}

impl Kind for Stage {
    fn kind(self) -> &'static str {
        match self {
            Self::Fetch => "fetch",
            Self::Discover => "discover",
            Self::Source => "source",
            Self::Decide => "decide",
            Self::Image => "image",
            Self::Render => "render",
            Self::Write => "write",
        }
    }
}

/// What one `unit` span is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Unit {
    /// A file read through a format.
    File,
    /// A page rendered.
    Page,
    /// A generated file: a feed, the sitemap.
    Generated,
    /// A raster image prepared.
    Image,
}

impl Kind for Unit {
    fn kind(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Page => "page",
            Self::Generated => "generated",
            Self::Image => "image",
        }
    }
}

/// A plugin slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Slot {
    /// A source plugin.
    Source,
    /// A format plugin.
    Format,
    /// A theme plugin.
    Theme,
}

impl Kind for Slot {
    fn kind(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Format => "format",
            Self::Theme => "theme",
        }
    }
}

/// What a blob call does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Io {
    /// Reads.
    Read,
    /// Writes.
    Write,
    /// Removes.
    Remove,
    /// Asks whether it exists, or how big it is.
    Probe,
}

impl Kind for Io {
    fn kind(self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
            Self::Remove => "remove",
            Self::Probe => "probe",
        }
    }
}

/// A `command` span: one run of a verb of `ink`.
#[macro_export]
macro_rules! command {
    ($verb:expr) => {
        $crate::tracing::info_span!($crate::name::span::COMMAND, kind = $crate::Kind::kind($verb))
    };
}

/// A `build` span: one build, named by its id and what triggered it.
#[macro_export]
macro_rules! build {
    ($id:expr, $trigger:expr) => {
        $crate::tracing::info_span!(
            $crate::name::span::BUILD,
            build = $crate::Field::field($crate::Shown($id)),
            kind = $crate::Kind::kind($trigger),
        )
    };
}

/// A `stage` span: one [`Stage`](crate::Stage), and the step of the engine inside it.
#[macro_export]
macro_rules! stage {
    ($stage:expr, $step:expr) => {
        $crate::tracing::info_span!($crate::name::span::STAGE, kind = $crate::Kind::kind($stage), step = $step)
    };
}

/// A `unit` span: one file, page or image, named as the author knows it.
#[macro_export]
macro_rules! unit {
    ($unit:expr, $label:expr) => {
        $crate::tracing::debug_span!(
            $crate::name::span::UNIT,
            kind = $crate::Kind::kind($unit),
            unit = $crate::Field::field($crate::Shown($label)),
        )
    };
}

/// A `plugin` span: one call into a plugin. `fuel`, `took_us` and `memory` are recorded when it
/// ends.
#[macro_export]
macro_rules! plugin {
    ($slot:expr, $name:expr, $call:expr) => {
        $crate::tracing::debug_span!(
            $crate::name::span::PLUGIN,
            kind = $crate::Kind::kind($slot),
            plugin = $crate::Field::field($crate::Shown($name)),
            call = $call,
            fuel = $crate::tracing::field::Empty,
            took_us = $crate::tracing::field::Empty,
            memory = $crate::tracing::field::Empty,
        )
    };
}

/// A `request` span of the edge. `status`, `bytes` and `took_us` are recorded when it ends.
#[macro_export]
macro_rules! request {
    ($id:expr, $method:expr, $path:expr) => {
        $crate::tracing::info_span!(
            $crate::name::span::REQUEST,
            request = $crate::Field::field($crate::Count($id)),
            method = $method,
            path = $crate::Field::field($crate::Shown($path)),
            host = $crate::tracing::field::Empty,
            status = $crate::tracing::field::Empty,
            bytes = $crate::tracing::field::Empty,
            took_us = $crate::tracing::field::Empty,
        )
    };
}

/// An `api` span: one call of the API, by method and route.
#[macro_export]
macro_rules! api {
    ($method:expr, $route:expr) => {
        $crate::tracing::info_span!(
            $crate::name::span::API,
            method = $method,
            route = $crate::Field::field($crate::Shown($route)),
            status = $crate::tracing::field::Empty,
        )
    };
}

/// A `job` span: one job of the control plane, by id, site and attempt.
#[macro_export]
macro_rules! job {
    ($id:expr, $site:expr, $attempt:expr) => {
        $crate::tracing::info_span!(
            $crate::name::span::JOB,
            job = $crate::Field::field($crate::Shown($id)),
            site = $crate::Field::field($crate::Shown($site)),
            attempt = $crate::Field::field($crate::Count(u64::from($attempt))),
        )
    };
}

/// A `statement` span: the shape of one typed statement, never its text or its values. `rows`
/// and `changes` are recorded when it ends.
#[macro_export]
macro_rules! statement {
    ($shape:expr) => {
        $crate::tracing::debug_span!(
            $crate::name::span::STATEMENT,
            kind = $crate::Kind::kind($shape),
            rows = $crate::tracing::field::Empty,
            changes = $crate::tracing::field::Empty,
        )
    };
}

/// A `blob` span: one read, write or removal, by the digest's first bytes.
#[macro_export]
macro_rules! blob {
    ($io:expr, $digest:expr) => {
        $crate::tracing::debug_span!(
            $crate::name::span::BLOB,
            kind = $crate::Kind::kind($io),
            digest = $crate::Field::field($crate::Digest($digest)),
            bytes = $crate::tracing::field::Empty,
        )
    };
}
