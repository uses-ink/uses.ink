//! The budgets of one control plane (decision 0088): the numbers every refusal reads and the
//! public limits page shows. One typed value, so a number is written once.

use std::fmt;
use std::time::Duration;

use model::Bytes;

use crate::Refusal;

/// What a budget limits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Budget {
    /// The bytes an account stores in all its sites.
    Stored,
    /// The builds an account starts in a day.
    Builds,
    /// The sites an account has.
    Sites,
    /// The deploys of a site that are kept: the newest, and the live one.
    Deploys,
    /// How long one build may take.
    BuildTime,
}

impl Budget {
    /// Every budget, in the order the limits page lists them.
    pub const ALL: [Self; 5] = [Self::Stored, Self::Builds, Self::Sites, Self::Deploys, Self::BuildTime];

    /// What the limits page calls it.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Stored => "Stored per account",
            Self::Builds => "Builds a day per account",
            Self::Sites => "Sites per account",
            Self::Deploys => "Deploys kept per site",
            Self::BuildTime => "Time for one build",
        }
    }

    /// What happens at the limit, as the page says it.
    pub const fn rule(self) -> &'static str {
        match self {
            Self::Stored => {
                "A deploy that would take the account over this is refused whole: nothing of it is published."
            }
            Self::Builds => {
                "A build requested over this waits for the next day: the request is refused, and no build is queued."
            }
            Self::Sites => "A new site over this is refused; the sites the account has are unchanged.",
            Self::Deploys => "Older deploys are removed, with the files only they used, when a newer one is published.",
            Self::BuildTime => "A build is given this long; the queue gives it to no one else before.",
        }
    }
}

/// A limit's value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Amount {
    /// A size.
    Bytes(Bytes),
    /// A number of things.
    Count(u64),
    /// A time.
    Time(Duration),
}

impl fmt::Display for Amount {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bytes(bytes) => bytes.fmt(out),
            Self::Count(count) => count.fmt(out),
            Self::Time(time) if time.as_secs() % 60 == 0 && !time.is_zero() => {
                write!(out, "{} minutes", time.as_secs() / 60)
            }
            Self::Time(time) => write!(out, "{} seconds", time.as_secs()),
        }
    }
}

/// What a budget refuses with: the three a request can be over.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Spent {
    /// More bytes stored than the account may.
    Stored,
    /// More builds in a day than the account may.
    Builds,
    /// More sites than the account may.
    Sites,
}

impl Spent {
    /// The code of the catalog that words a refusal of this budget.
    pub const fn code(self) -> &'static str {
        match self {
            Self::Stored => "budget.stored.over",
            Self::Builds => "budget.builds.over",
            Self::Sites => "budget.sites.over",
        }
    }

    /// The budget it is of.
    pub const fn budget(self) -> Budget {
        match self {
            Self::Stored => Budget::Stored,
            Self::Builds => Budget::Builds,
            Self::Sites => Budget::Sites,
        }
    }
}

/// The operator's limits. Every field has a default and a flag of `ink serve`; the limits page
/// and the refusals both read these values and no others.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// Bytes an account stores across its sites. Default 1 GiB.
    pub stored: Bytes,
    /// Builds an account starts in 24 hours. Default 60.
    pub builds: u64,
    /// Sites an account has. Default 10.
    pub sites: u64,
    /// Deploys of a site kept, the live one included. Default 20.
    pub deploys: u64,
    /// How long one build may take. Default 10 minutes.
    pub time: Duration,
}

impl Limits {
    /// The defaults: the owner's numbers of `docs/hosted.md` HQ8 (1 GiB, 60 builds a day, 10
    /// minutes a build, 20 deploys kept) and 10 sites, which is this package's placeholder.
    pub const DEFAULT: Self =
        Self { stored: Bytes(1024 * 1024 * 1024), builds: 60, sites: 10, deploys: 20, time: Duration::from_mins(10) };

    /// The value of `budget`.
    pub const fn amount(&self, budget: Budget) -> Amount {
        match budget {
            Budget::Stored => Amount::Bytes(self.stored),
            Budget::Builds => Amount::Count(self.builds),
            Budget::Sites => Amount::Count(self.sites),
            Budget::Deploys => Amount::Count(self.deploys),
            Budget::BuildTime => Amount::Time(self.time),
        }
    }

    /// Every budget with its value, in the order of [`Budget::ALL`]: what the limits page lists.
    pub fn list(&self) -> Vec<(Budget, Amount)> {
        Budget::ALL.into_iter().map(|budget| (budget, self.amount(budget))).collect()
    }

    /// The refusal for `value` of `spent`, with the limit this value of `Limits` has for it.
    pub fn over(&self, spent: Spent, value: u64) -> Refusal {
        let limit = match self.amount(spent.budget()) {
            Amount::Bytes(bytes) => bytes.count(),
            Amount::Count(count) => count,
            Amount::Time(time) => time.as_secs(),
        };
        Refusal::Over { spent, value, limit }
    }
}

impl Default for Limits {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_refusal_carries_the_value_the_list_shows() {
        let limits = Limits { stored: Bytes(5), builds: 2, ..Limits::DEFAULT };
        assert_eq!(limits.over(Spent::Stored, 9), Refusal::Over { spent: Spent::Stored, value: 9, limit: 5 });
        assert_eq!(limits.over(Spent::Builds, 2), Refusal::Over { spent: Spent::Builds, value: 2, limit: 2 });
        assert_eq!(limits.list().len(), Budget::ALL.len());
        assert_eq!(limits.list()[1], (Budget::Builds, Amount::Count(2)));
    }

    #[test]
    fn amounts_read_as_a_person_says_them() {
        assert_eq!(Amount::Bytes(Limits::DEFAULT.stored).to_string(), "1 GiB");
        assert_eq!(Amount::Time(Limits::DEFAULT.time).to_string(), "10 minutes");
        assert_eq!(Amount::Time(Duration::from_secs(90)).to_string(), "90 seconds");
        assert_eq!(Amount::Count(60).to_string(), "60");
    }
}
