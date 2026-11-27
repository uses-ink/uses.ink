//! The words the dashboard uses for the control plane's values, written once.

use std::fmt;

use build::Trigger;
use control::{JobState, Purpose, SiteState};
use model::{Ability, DeployId};

/// A value shown in the dashboard's words.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Word<T>(pub(crate) T);

impl fmt::Display for Word<JobState> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self.0 {
            JobState::Queued => "queued",
            JobState::Running => "building",
            JobState::Succeeded => "published",
            JobState::Refused => "refused: the site does not build",
            JobState::Failed => "failed: the builder gave up",
        })
    }
}

impl fmt::Display for Word<Trigger> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self.0 {
            Trigger::Manual => "asked for by hand",
            Trigger::Push => "a push",
            Trigger::Poll => "the poll saw a change",
            Trigger::Upload => "an upload",
            _ => "a trigger this version cannot show",
        })
    }
}

impl fmt::Display for Word<Ability> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self.0 {
            Ability::Snapshot => "snapshot",
            Ability::History => "history",
            Ability::Watch => "watch",
            Ability::Private => "private",
            _ => "an ability this version cannot show",
        })
    }
}

impl fmt::Display for Word<Purpose> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self.0 {
            Purpose::GitToken => "token for a private git repository",
            Purpose::WebhookSecret => "secret that signs a forge's webhooks",
            _ => "a use this version cannot show",
        })
    }
}

impl fmt::Display for Word<SiteState> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self.0 {
            SiteState::Active => "active",
            SiteState::Removing => "being removed",
            _ => "in a state this version cannot show",
        })
    }
}

/// A deploy id as people write it: its first twelve digits.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Brief(pub(crate) DeployId);

impl fmt::Display for Brief {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let whole = self.0.to_string();
        out.write_str(whole.get(..12).unwrap_or(&whole))
    }
}
