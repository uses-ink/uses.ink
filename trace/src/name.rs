//! Every span and event name, as constants: a name is never typed twice, so a test and a call
//! site cannot drift apart.
//!
//! Field keys are identifiers in the macros, which `tracing` requires, so they cannot be
//! constants. They are listed in docs/tracing.md instead, and the layers treat `kind` as the
//! key that tells apart spans of one name.

/// The names of spans, outermost layer first.
pub mod span {
    /// One run of an `ink` verb.
    pub const COMMAND: &str = "command";
    /// One build of one site.
    pub const BUILD: &str = "build";
    /// One stage of a build.
    pub const STAGE: &str = "stage";
    /// One file, page or image.
    pub const UNIT: &str = "unit";
    /// One call into a plugin.
    pub const PLUGIN: &str = "plugin";
    /// One request the edge answers.
    pub const REQUEST: &str = "request";
    /// One API call.
    pub const API: &str = "api";
    /// One job of the control plane.
    pub const JOB: &str = "job";
    /// One database statement.
    pub const STATEMENT: &str = "statement";
    /// One blob read or write.
    pub const BLOB: &str = "blob";
}

/// The names of events.
pub mod event {
    /// A build began.
    pub const BUILD_START: &str = "build.start";
    /// A build ended, with its status.
    pub const BUILD_END: &str = "build.end";
    /// The engine finished interpreting: counts of what it decided and said.
    pub const INTERPRETED: &str = "engine.interpreted";
    /// The engine finished rendering: counts of units reused and rendered.
    pub const RENDERED: &str = "engine.rendered";
    /// A folder or a repository was opened as a source.
    pub const SOURCE_OPENED: &str = "source.opened";
    /// A source gave a snapshot.
    pub const SOURCE_FETCHED: &str = "source.fetched";
    /// The planner decided what to do with one unit, and why.
    pub const UNIT_PLANNED: &str = "unit.planned";
    /// The unit cache answered, or did not.
    pub const CACHE_LOOKUP: &str = "cache.lookup";
    /// An image was decoded.
    pub const IMAGE_DECODED: &str = "image.decoded";
    /// An image was resized.
    pub const IMAGE_RESIZED: &str = "image.resized";
    /// An image was encoded.
    pub const IMAGE_ENCODED: &str = "image.encoded";
    /// An image was refused.
    pub const IMAGE_REFUSED: &str = "image.refused";
    /// A plugin call ended.
    pub const PLUGIN_CALL: &str = "plugin.call";
    /// A plugin call was stopped: fuel, time, memory, a table, a trap.
    pub const PLUGIN_REFUSED: &str = "plugin.refused";
    /// A plugin was compiled or loaded from the cache.
    pub const PLUGIN_LOADED: &str = "plugin.loaded";
    /// The edge listens.
    pub const EDGE_LISTENING: &str = "edge.listening";
    /// A connection was accepted.
    pub const EDGE_ACCEPTED: &str = "edge.accepted";
    /// A request was answered.
    pub const REQUEST_ANSWERED: &str = "request.answered";
    /// A request was refused.
    pub const REQUEST_REFUSED: &str = "request.refused";
    /// A request failed inside the edge.
    pub const REQUEST_FAILED: &str = "request.failed";
    /// A response was cut short.
    pub const RESPONSE_CUT: &str = "response.cut";
    /// A connection failed.
    pub const CONNECTION_FAILED: &str = "connection.failed";
    /// A closed connection was drained before the socket went away.
    pub const CONNECTION_LINGERED: &str = "connection.lingered";
    /// The edge stops accepting, then stops.
    pub const EDGE_STOPPING: &str = "edge.stopping";
    /// The edge stopped.
    pub const EDGE_STOPPED: &str = "edge.stopped";
    /// A site's content was reloaded.
    pub const EDGE_RELOADED: &str = "edge.reloaded";
    /// An API route answered.
    pub const API_ANSWERED: &str = "api.answered";
    /// The API decided who is calling, without saying the token.
    pub const API_AUTH: &str = "api.auth";
    /// A job was claimed.
    pub const JOB_CLAIMED: &str = "job.claimed";
    /// A job was dropped without running.
    pub const JOB_DROPPED: &str = "job.dropped";
    /// A job was settled.
    pub const JOB_SETTLED: &str = "job.settled";
    /// A job failed and will be tried again, or has no tries left.
    pub const JOB_FAILED: &str = "job.failed";
    /// A deploy became live.
    pub const DEPLOY_PUBLISHED: &str = "deploy.published";
    /// A site went back to an earlier deploy.
    pub const DEPLOY_ROLLED: &str = "deploy.rolled";
    /// A migration was applied.
    pub const DB_MIGRATED: &str = "db.migrated";
    /// A statement ran.
    pub const DB_STATEMENT: &str = "db.statement";
    /// A batch of statements ran in one transaction.
    pub const DB_BATCH: &str = "db.batch";
    /// A blob was read, written or removed.
    pub const BLOB_IO: &str = "blob.io";
    /// The watch asked where a source is.
    pub const WATCH_POLLED: &str = "watch.polled";
    /// The watch queued a build.
    pub const WATCH_QUEUED: &str = "watch.queued";
    /// The worker looked for a job.
    pub const WORKER_TICK: &str = "worker.tick";
    /// A command ended.
    pub const COMMAND_END: &str = "command.end";
}
