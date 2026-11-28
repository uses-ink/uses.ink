//! `cargo xtask release <step>`: the release steps that need no account and publish nothing, run
//! on a build host through the farm (decision 0170). `plan` and `build` are `dist` over the pinned
//! config; `image` builds the self-host image, starts it the way the guide says and probes it.

mod dist;
mod fault;
mod image;

use std::fmt;
use std::path::Path;
use std::str::FromStr;

use crate::check::Step;
use crate::error::Error;
use crate::report::Report;

pub(crate) use fault::Fault;

/// One step of the release that can be run without publishing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Release {
    /// `dist plan`: the artifacts the tag would build, and no error.
    Plan,
    /// The components, then `dist build --artifacts=local` for this host's Linux target.
    Build,
    /// The container image built, started read-only as the guide says, and probed.
    Image,
}

impl Release {
    /// Every step, in the order they are listed.
    pub(crate) const ALL: [Self; 3] = [Self::Plan, Self::Build, Self::Image];

    /// Its name on the command line.
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Plan => "plan",
            Self::Build => "build",
            Self::Image => "image",
        }
    }
}

impl Step for Release {
    fn run(&self, root: &Path, report: &mut Report) {
        match self {
            Self::Plan => dist::Dist::Plan.run(root, report),
            Self::Build => dist::Dist::Build.run(root, report),
            Self::Image => image::Image.run(root, report),
        }
    }
}

impl fmt::Display for Release {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(self.name())
    }
}

impl FromStr for Release {
    type Err = Error;

    fn from_str(given: &str) -> Result<Self, Error> {
        Self::ALL
            .into_iter()
            .find(|step| step.name() == given)
            .ok_or_else(|| Error::Release { given: given.to_owned() })
    }
}
