//! The command line, as clap reads it.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

/// A folder becomes a site.
#[derive(Debug, Parser)]
#[command(name = "ink", version, about)]
pub struct Cli {
    /// Print the result as one JSON document, and events as NDJSON on stderr.
    #[arg(long, global = true)]
    pub json: bool,
    /// Also print notices: defaults chosen, files skipped. They are warn-level quiet otherwise.
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
    /// Build the folder in memory and serve it on 127.0.0.1.
    Preview(Preview),
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
}

/// `ink check <dir>`.
#[derive(Debug, Args)]
pub struct Check {
    /// The site's folder.
    pub dir: PathBuf,
    /// Exit with 6 when there are warnings.
    #[arg(long)]
    pub strict: bool,
}

/// `ink explain [--pin] [path]`.
#[derive(Debug, Args)]
pub struct Explain {
    /// A file of the folder, as a path in it: only its decisions. Without one, every decision.
    pub path: Option<String>,
    /// Print the decisions as `ink.toml` pins instead: every one, or the file's with a path.
    #[arg(long)]
    pub pin: bool,
    /// The site's folder.
    #[arg(short = 'C', long, default_value = ".")]
    pub dir: PathBuf,
}

/// `ink preview <dir>`.
#[derive(Debug, Args)]
pub struct Preview {
    /// The site's folder.
    pub dir: PathBuf,
    /// The port on 127.0.0.1.
    #[arg(long, default_value_t = 4000)]
    pub port: u16,
}
