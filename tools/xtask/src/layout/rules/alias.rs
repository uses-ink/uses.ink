//! The rule over aliases: a crate another member uses is named through the workspace's alias.

use crate::layout::model::Workspace;
use crate::layout::{Problem, Problems, Rule};

/// A crate another member uses is named through the workspace's alias.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Alias;

impl Rule for Alias {
    const NAME: &'static str = "alias";

    fn check(&self, workspace: &Workspace, problems: &mut Problems) {
        let used: Vec<&str> =
            workspace.crates.iter().flat_map(|c| c.dependencies.iter().map(|d| d.name.as_str())).collect();
        for krate in &workspace.crates {
            if used.contains(&krate.name.as_str()) && !workspace.aliased.contains(&krate.name) {
                problems.push(Problem::Alias { krate: krate.name.clone() });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::model::Crate;

    #[test]
    fn a_crate_another_uses_needs_an_alias() {
        let crates = || {
            vec![
                Crate::sample("uses-ink-model", "model", None, &[]),
                Crate::sample("uses-ink-engine", "engine", None, &[("uses-ink-model", None)]),
            ]
        };
        let found = Problems::of(&Alias, &Workspace::sample(crates(), &[]));
        assert!(matches!(found.as_slice(), [Problem::Alias { krate }] if krate == "uses-ink-model"));
        assert!(Problems::of(&Alias, &Workspace::sample(crates(), &["uses-ink-model"])).is_empty());
    }
}
