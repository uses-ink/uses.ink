//! The command line, as clap parses it. What `ink --help`, the completions and the manual say
//! about it lives in the command tree (`crate::grammar::tree`), which decorates the command these types
//! derive: its groups, wording, aliases, examples, exit codes and environment.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::deploy::{List, Rollback, Show};
use crate::grammar::tree::Heading;

/// A folder becomes a site.
#[derive(Debug, Parser)]
#[command(name = "ink", version, about, disable_help_subcommand = true, subcommand_required = true)]
#[command(arg_required_else_help = true, next_help_heading = Heading::GLOBAL)]
pub struct Cli {
    /// The site's folder [default: .]
    #[arg(short = 'C', long, global = true, value_name = "DIR", env = "INK_DIR")]
    pub dir: Option<PathBuf>,
    /// One JSON document on stdout, diagnostics as NDJSON on stderr.
    #[arg(long, global = true)]
    pub json: bool,
    /// Say less: `-q` drops stage lines and notices and keeps warnings, the result and what a build
    /// makes public; `-qq` leaves only errors.
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    pub quiet: u8,
    /// Say more: -v adds notices, -vv adds every file.
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,
    /// When to colour output, decided for each stream: `auto` follows `NO_COLOR`, `CLICOLOR`,
    /// `CLICOLOR_FORCE`, `TERM` and whether the stream is a terminal [default: auto].
    #[arg(long, global = true, value_enum, value_name = "WHEN", env = "INK_COLOR")]
    pub color: Option<When>,
    /// Print log lines only, with no live redraw of progress.
    #[arg(long, global = true, env = "INK_NO_PROGRESS")]
    pub no_progress: bool,
    /// Log to stderr as `ink=debug,edge=trace` says: areas (ink, engine, build, edge, control, db,
    /// blob, host, plugin, watch, queue, cli) or targets, at a level. Off unless this, `-v` or
    /// `INK_LOG` asks.
    #[arg(long, global = true, value_name = "FILTER", env = "INK_LOG", help_heading = Heading::DIAGNOSTICS)]
    pub log: Option<logging::Filter>,
    /// Also write the log as JSON lines to this file.
    #[arg(long, global = true, value_name = "PATH", env = "INK_LOG_FILE", help_heading = Heading::DIAGNOSTICS)]
    pub log_file: Option<PathBuf>,
    /// What to do.
    #[command(subcommand)]
    pub command: Command,
}

/// When output is coloured: `--color`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, ValueEnum, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum When {
    /// Colour a stream that is a terminal and nothing else asks otherwise.
    #[default]
    Auto,
    /// Colour every stream.
    Always,
    /// Colour none.
    Never,
}

/// What `ink` does.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Link this folder to a site, without publishing it.
    Init(crate::command::publish::Init),
    /// Read the folder and report what is wrong; writes nothing.
    Check(Check),
    /// Build the folder into an output folder and print its deploy id.
    Build(Build),
    /// Build in memory and serve it on 127.0.0.1 while you write.
    Preview(Preview),
    /// Serve a built deploy the way production does.
    Serve(Serve),
    /// Show the decisions behind the site, or behind one file.
    Explain(Explain),
    /// Write one decision's value into ink.toml.
    Pin(PinArgs),
    /// List what is published and why, or choose what to publish.
    Select(Select),
    /// Sign in to uses.ink.
    Login(crate::command::auth::Login),
    /// Sign out.
    Logout(crate::command::auth::Server),
    /// Show who you are signed in as.
    Whoami(crate::command::auth::Server),
    /// List, make and end keys.
    Key(crate::command::auth::Key),
    /// Read abuse reports, suspend sites and lift suspensions.
    Abuse(crate::command::abuse::Abuse),
    /// `ink org`.
    Org(crate::command::org::Org),
    /// Send the folder to its site and start a deploy.
    Push(crate::command::publish::Push),
    /// Show what is live and what is building.
    Status(crate::command::publish::Status),
    /// Wait for the next deploy of this commit and report it.
    Watch(crate::command::publish::Watch),
    /// List, read, roll back and resume deploys.
    Deploy(Deploy),
    /// `ink deploy list`, as `research/flow.md` spells it.
    #[command(hide = true)]
    Deploys(List),
    /// Claim, prove, list and remove custom domains.
    Domain(crate::command::domain::Domain),
    /// Write the whole site, source and history, to a folder.
    Export(crate::command::publish::Export),
    /// Print a completion script for your shell.
    Completions(Completions),
    /// Print the manual page.
    Man,
    /// Show this message, or the help of one command.
    Help(Topic),
}

/// `ink deploy <list|show|rollback|resume>`: a group with no behaviour of its own.
#[derive(Debug, Args)]
pub struct Deploy {
    /// What to do with the deploys.
    #[command(subcommand)]
    pub command: Deployment,
}

/// What `ink deploy` does.
#[derive(Debug, Subcommand)]
pub enum Deployment {
    /// History, newest first.
    List(List),
    /// One deploy: its build and its log.
    Show(Show),
    /// Make an earlier deploy the live one, and pause new deploys.
    Rollback(Rollback),
    /// Let the site deploy new pushes again after a rollback.
    Resume(crate::deploy::Remote),
}

/// `ink completions <SHELL>`.
#[derive(Debug, Args)]
pub struct Completions {
    /// The shell to print the script for.
    #[arg(value_enum)]
    pub shell: crate::grammar::completions::Shell,
}

/// `ink help [COMMAND | exit | environment | CODE]`.
#[derive(Debug, Args)]
pub struct Topic {
    /// A command (`deploy rollback`), `exit`, `environment`, or a diagnostic or decision code.
    #[arg(value_name = "TOPIC")]
    pub words: Vec<String>,
}

/// Where a site's files are read from: the folder as it is on disk, or a git repository at
/// one commit.
#[derive(Clone, Debug, Default, Args)]
#[command(next_help_heading = Heading::SOURCE)]
pub struct Origin {
    /// `folder` reads the files on disk, `git` reads a commit [default: `git` when .git exists,
    /// said as a notice].
    #[arg(long, value_enum, value_name = "KIND")]
    pub source: Option<Kind>,
    /// The commit to read, as `git rev-parse` takes it [default: HEAD]. Only for a git
    /// repository.
    #[arg(long, value_name = "REV")]
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

/// `ink build [-o <out>]`.
#[derive(Debug, Args)]
pub struct Build {
    /// The site's folder, from `-C`.
    #[arg(skip)]
    pub dir: PathBuf,
    /// Where to write the site [default: .ink/out].
    #[arg(short, long, value_name = "DIR", help_heading = Heading::OUTPUT)]
    pub out: Option<PathBuf>,
    /// The address the site is served at, for feeds and canonical links.
    #[arg(long, value_name = "URL", help_heading = Heading::OUTPUT)]
    pub base: Option<String>,
    /// Exit 6 on warnings, and write nothing.
    #[arg(long, help_heading = Heading::OUTPUT)]
    pub strict: bool,
    /// Where to keep work between builds, such as image copies [default: .ink/cache].
    #[arg(long, value_name = "DIR", help_heading = Heading::WORK)]
    pub cache: Option<PathBuf>,
    /// Where the files are read from.
    #[command(flatten)]
    pub origin: Origin,
    /// Build natively and through the wasm components, write nothing, and fail (exit 1) when the
    /// two deploys differ.
    #[arg(long, help_heading = Heading::VERIFICATION)]
    pub check: bool,
    /// A folder whose components replace the ones `ink` carries when `--check` runs them:
    /// `format-markdown.wasm` (or `format-obsidian.wasm` for a vault) and `theme-quiet.wasm`, as
    /// `cargo xtask component` writes them. A file the folder lacks is the bundled one; each
    /// component run says which it used (`-v`, or `components` in `--json`). Compiled components
    /// are kept in `components/` of the cache.
    #[arg(long, requires = "check", value_name = "DIR", help_heading = Heading::VERIFICATION)]
    pub components: Option<PathBuf>,
}

/// `ink check`.
#[derive(Debug, Args)]
pub struct Check {
    /// The site's folder, from `-C`.
    #[arg(skip)]
    pub dir: PathBuf,
    /// Exit with 6 when there are warnings.
    #[arg(long)]
    pub strict: bool,
    /// Where the files are read from.
    #[command(flatten)]
    pub origin: Origin,
}

/// `ink explain [path]` and `ink explain [path] --why <code>`.
///
/// A decision is addressed `[PATH] CODE`: the path is the positional plain `explain` shares, the
/// code rides on `--why`. [`Explain::address`] folds the two into the one list [`crate::command::explain::asked`]
/// reads.
#[derive(Debug, Args)]
pub struct Explain {
    /// A file of the folder, as a path in it: only its decisions. Without one, every decision.
    pub path: Option<String>,
    /// Replay one decision's ladder, rung by rung: `ink explain posts/a.md --why page.title`, or
    /// a site code alone (`--why site.title`).
    #[arg(long, num_args = 1, value_name = "CODE")]
    pub why: Option<Vec<String>>,
    /// The site's folder, from `-C`.
    #[arg(skip)]
    pub dir: PathBuf,
    /// Where the files are read from.
    #[command(flatten)]
    pub origin: Origin,
}

/// `ink pin [path] <code>`.
#[derive(Debug, Args)]
pub struct PinArgs {
    /// A decision, addressed `[PATH] CODE`: a file's path and a code (`posts/a.md page.title`), or
    /// a site code alone (`site.title`).
    #[arg(required = true, num_args = 1..=2, value_names = ["PATH", "CODE"])]
    pub words: Vec<String>,
    /// Print the pin and write nothing.
    #[arg(long)]
    pub dry_run: bool,
    /// The site's folder, from `-C`.
    #[arg(skip)]
    pub dir: PathBuf,
    /// Where the files are read from.
    #[command(flatten)]
    pub origin: Origin,
}

/// `ink preview`: serve the folder on 127.0.0.1, or read and end the previews of branches.
#[derive(Debug, Args)]
#[command(args_conflicts_with_subcommands = true)]
pub struct Preview {
    /// What to do with the previews of branches that the watch of a server built, instead.
    #[command(subcommand)]
    pub command: Option<crate::command::preview::Branches>,
    /// The site's folder, from `-C`.
    #[arg(skip)]
    pub dir: PathBuf,
    /// The port on 127.0.0.1.
    #[arg(long, default_value_t = 4000)]
    pub port: u16,
    /// Where to keep work between builds, such as image copies [default: .ink/cache].
    #[arg(long, value_name = "DIR", help_heading = Heading::WORK)]
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
    /// The site's folder, from `-C`.
    #[arg(skip)]
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
    /// host name [default: .ink/out of the site].
    pub folder: Option<PathBuf>,
    /// The address to listen on.
    #[arg(long = "addr", default_value = "127.0.0.1:8080", help_heading = Heading::SERVER)]
    pub address: std::net::SocketAddr,
    /// How a request's host picks a deploy.
    #[arg(long, value_enum, default_value_t, help_heading = Heading::SERVER)]
    pub host_mode: HostMode,
    /// Turns on the control plane and its API. Its database, blobs, sealing key and the
    /// operator's public keys live in this folder. Without it only the folder is served.
    #[arg(long, help_heading = Heading::SERVER)]
    pub data: Option<PathBuf>,
    /// The host name whose requests are API calls, signed with a key (`ink login`).
    #[arg(long, default_value = "api.localhost", help_heading = Heading::SERVER)]
    pub api_host: String,
    /// Where sites are served, for the addresses the API reports (`<handle>.<host>`) and the
    /// origin passkeys are made for: give the address a browser uses to reach this server.
    /// [default: `http://localhost:` and the port the server listens on, which the start says]
    #[arg(long, help_heading = Heading::SERVER)]
    pub origin: Option<String>,
    /// The largest JSON request body, in bytes. [default: 65536]
    #[arg(long, help_heading = Heading::LIMITS)]
    pub api_body: Option<u64>,
    /// The largest uploaded archive, in bytes. [default: 268435456]
    #[arg(long, help_heading = Heading::LIMITS)]
    pub upload_bytes: Option<u64>,
    /// The most files in an uploaded archive. [default: 10000]
    #[arg(long, help_heading = Heading::LIMITS)]
    pub upload_files: Option<u64>,
    /// The largest file in an uploaded archive, in bytes. [default: 67108864]
    #[arg(long, help_heading = Heading::LIMITS)]
    pub upload_file: Option<u64>,
    /// The largest tree `ink push` may send, in bytes; the tree is read whole. [default: 8388608]
    #[arg(long, help_heading = Heading::LIMITS)]
    pub upload_tree: Option<u64>,
    /// The most bytes of files in one `ink push` request, held in memory while it is checked and
    /// stored. [default: 67108864]
    #[arg(long, help_heading = Heading::LIMITS)]
    pub upload_part: Option<u64>,
    /// The largest exported archive, in bytes. [default: 268435456]
    #[arg(long, help_heading = Heading::LIMITS)]
    pub export_bytes: Option<u64>,
    /// Seconds between polls of a git source. At least 10. [default: 600]
    #[arg(long, help_heading = Heading::LIMITS)]
    pub poll_interval: Option<u64>,
    /// Seconds a source that does not change is backed off to. [default: 3600]
    #[arg(long, help_heading = Heading::LIMITS)]
    pub poll_max: Option<u64>,
    /// Sign-in: mail, lifetimes, who may sign up and the limits on asking for a link.
    #[command(flatten)]
    pub signin: Box<crate::selfhost::signin::Flags>,
    /// Keys: the audience, the skew window, how long keys live and how enrolling is limited.
    #[command(flatten)]
    pub auth: Box<crate::command::auth::Flags>,
    /// How the lookups and loops behind the API run.
    #[command(flatten)]
    pub hosting: Box<crate::selfhost::hosting::Hosting>,
}
