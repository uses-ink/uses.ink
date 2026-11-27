//! The command line, as clap reads it.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

/// A folder becomes a site.
#[derive(Debug, Parser)]
#[command(name = "ink", version, about)]
pub struct Cli {
    /// Print the result as one JSON document on stdout, an error's too, and diagnostics as NDJSON
    /// events on stderr.
    #[arg(long, global = true)]
    pub json: bool,
    /// Also print notices (defaults chosen, folders skipped), as text or, with `--json`, events.
    /// They are not printed otherwise.
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,
    /// What to do.
    #[command(subcommand)]
    pub command: Command,
}

/// What `ink` does.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Build the folder into an output folder and print the deploy id.
    Build(Build),
    /// Interpret the folder and report its diagnostics, writing nothing.
    Check(Check),
    /// Show the decisions behind the site, or one file's.
    Explain(Explain),
    /// Write one decision's current value into ink.toml as a pin.
    Pin(Pin),
    /// Build the folder in memory and serve it on 127.0.0.1.
    Preview(Preview),
    /// List what is published and why, or record a selection of what to publish.
    Select(Select),
    /// Serve a built deploy through the edge the product runs, with its production headers.
    Serve(Serve),
}

/// Where a site's files are read from: the folder as it is on disk, or a git repository at
/// one commit.
#[derive(Clone, Debug, Default, Args)]
pub struct Origin {
    /// Read the files on disk (`folder`) or a commit of the repository (`git`)
    /// [default: `git` when <dir>/.git exists, said as a notice, else `folder`].
    #[arg(long, value_enum)]
    pub source: Option<Kind>,
    /// The commit to read, as `git rev-parse` takes it: a branch, a tag, an id, `HEAD~2`
    /// [default: HEAD]. Only for a git repository.
    #[arg(long)]
    pub rev: Option<String>,
}

/// What kind of source a folder is read as.
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Kind {
    /// The files on disk, uncommitted changes included, with file times as bounds.
    Folder,
    /// The tree of one commit, with dates from the history behind it.
    Git,
}

/// `ink build <dir> [-o <out>]`.
#[derive(Debug, Args)]
pub struct Build {
    /// The site's folder.
    pub dir: PathBuf,
    /// Where to write the site [default: <dir>/.ink/out, which no command reads as content].
    #[arg(short, long)]
    pub out: Option<PathBuf>,
    /// The address the site is served at, for feeds, the sitemap and canonical links.
    #[arg(long)]
    pub base: Option<String>,
    /// Exit with 6 when there are warnings, writing nothing.
    #[arg(long)]
    pub strict: bool,
    /// Where to keep work between builds, such as image copies [default: <dir>/.ink/cache,
    /// which no command reads as content].
    #[arg(long, value_name = "DIR")]
    pub cache: Option<PathBuf>,
    /// Where the files are read from.
    #[command(flatten)]
    pub origin: Origin,
    /// Build natively and through the plugins as wasm components, write nothing, and fail
    /// (exit 1) when the two deploys differ.
    #[arg(long)]
    pub check: bool,
    /// A folder whose components replace the ones `ink` carries when `--check` runs them:
    /// `format-markdown.wasm` (or `format-obsidian.wasm` for a vault) and `theme-quiet.wasm`, as
    /// `cargo xtask component` writes them. A file the folder lacks is the bundled one; each
    /// component run says which it used (`-v`, or `components` in `--json`). Compiled components
    /// are kept in `components/` of the cache.
    #[arg(long, requires = "check")]
    pub components: Option<PathBuf>,
}

/// `ink check <dir>`.
#[derive(Debug, Args)]
pub struct Check {
    /// The site's folder.
    pub dir: PathBuf,
    /// Exit with 6 when there are warnings.
    #[arg(long)]
    pub strict: bool,
    /// Where the files are read from.
    #[command(flatten)]
    pub origin: Origin,
}

/// `ink explain [path]` and `ink explain --why [path] <code>`.
#[derive(Debug, Args)]
pub struct Explain {
    /// A file of the folder, as a path in it: only its decisions. Without one, every decision.
    #[arg(conflicts_with = "why")]
    pub path: Option<String>,
    /// Replay one decision's ladder, rung by rung: a file's path and a code (`posts/a.md
    /// page.title`), or a site code alone (`site.title`).
    #[arg(long, num_args = 1..=2, value_names = ["PATH", "CODE"])]
    pub why: Option<Vec<String>>,
    /// The site's folder.
    #[arg(short = 'C', long, default_value = ".")]
    pub dir: PathBuf,
    /// Where the files are read from.
    #[command(flatten)]
    pub origin: Origin,
}

/// `ink pin [path] <code>`.
#[derive(Debug, Args)]
pub struct Pin {
    /// A file's path and a code (`posts/a.md page.title`), or a site code alone (`site.title`).
    #[arg(required = true, num_args = 1..=2, value_names = ["PATH", "CODE"])]
    pub words: Vec<String>,
    /// Print the pin and write nothing.
    #[arg(long)]
    pub dry_run: bool,
    /// The site's folder.
    #[arg(short = 'C', long, default_value = ".")]
    pub dir: PathBuf,
    /// Where the files are read from.
    #[command(flatten)]
    pub origin: Origin,
}

/// `ink preview <dir>`.
#[derive(Debug, Args)]
pub struct Preview {
    /// The site's folder.
    pub dir: PathBuf,
    /// The port on 127.0.0.1.
    #[arg(long, default_value_t = 4000)]
    pub port: u16,
    /// Where to keep work between builds, such as image copies [default: <dir>/.ink/cache,
    /// which no command reads as content].
    #[arg(long, value_name = "DIR")]
    pub cache: Option<PathBuf>,
    /// Where the files are read from.
    #[command(flatten)]
    pub origin: Origin,
}

/// `ink select [path...]`, `ink select --folder <glob>` and `ink select --remove ...`.
///
/// It reads the files on disk, since it writes into them: a build of a git repository reads a
/// commit, so commit the selection before building that.
#[derive(Debug, Args)]
#[command(group = clap::ArgGroup::new("target").args(["paths", "folder"]).multiple(false))]
pub struct Select {
    /// Pages to select, as paths in the folder: `publish: true` is written into the frontmatter
    /// of each, which the file must already have. Without a path or `--folder`, every page is
    /// listed with whether it is published and why.
    pub paths: Vec<String>,
    /// Select every file a glob matches (`notes/**`) by adding it to `include` in ink.toml.
    #[arg(long, value_name = "GLOB")]
    pub folder: Option<String>,
    /// Take the selection back: remove `publish: true` from each page, or the glob from `include`.
    /// In a public source a page that is not selected is still published by default.
    #[arg(long, requires = "target")]
    pub remove: bool,
    /// Say what would be written and write nothing.
    #[arg(long)]
    pub dry_run: bool,
    /// The site's folder.
    #[arg(short = 'C', long, default_value = ".")]
    pub dir: PathBuf,
}

/// How `ink serve` maps a request's host to a deploy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum)]
pub enum HostMode {
    /// One deploy when the folder holds a manifest, else one folder per host.
    #[default]
    Auto,
    /// The folder is one deploy, served for every host name.
    Single,
    /// `<folder>/<host>` is the deploy of `<host>`; a host with no folder is no site.
    Folders,
}

/// `ink serve [folder]`: the deploy `ink build` wrote, served by the edge crate the product uses.
///
/// Unlike `ink preview`, which builds the site in memory on every start and serves it on
/// 127.0.0.1 for writing, this serves what `ink build` already wrote, through `edge` and
/// `edge-axum`: conditional and range requests, the host and path refusals, the request and
/// response budgets, and the manifest's headers as they are in production.
#[derive(Debug, Args)]
pub struct Serve {
    /// A folder `ink build` wrote into, or with `--host-mode folders` a folder of them, one per
    /// host name.
    #[arg(default_value = ".ink/out")]
    pub folder: PathBuf,
    /// The address to listen on.
    #[arg(long = "addr", default_value = "127.0.0.1:8080")]
    pub address: std::net::SocketAddr,
    /// How a request's host picks a deploy.
    #[arg(long, value_enum, default_value_t)]
    pub host_mode: HostMode,
}
