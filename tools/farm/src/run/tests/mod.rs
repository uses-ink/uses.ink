//! The verbs of [`Farm`] over a real shell and over a scripted fake.

mod docs;
mod fake;
mod reuse;
mod rig;
mod slots;
mod verbs;
mod wake;

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, SystemTime};

use crate::cache::Cache;
use crate::cli::console::Console;
use crate::cli::exit::Exit;
use crate::cli::name::HostName;
use crate::cli::{Action, Flags};
use crate::config::jobs::Settings;
use crate::config::tree::Tree;
use crate::host::fake::{Fake, Reply};
use crate::host::transport::{Exec, Link, Local};
use crate::host::{Fleet, Host, Member};
use crate::job::Verdict;
use crate::place::history;
use crate::place::least::LeastLoaded;
use crate::place::wake::Wake;
use crate::run::{Farm, Setup};
use crate::{cache, cli, host, job, place};

use fake::{Wakeup, asking, farm, host, probe, silent};
use rig::{Rig, again, counting, farm_tree, job, run_one, runs, within};
