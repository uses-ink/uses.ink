//! The rule over crate names: the path predicts the name (docs/conventions.md, Crates and
//! directories).

use crate::layout::model::{Crate, Workspace};
use crate::layout::{Problem, Problems, Rule};

/// Crates with a name of their own: product-independent, published as is, MIT OR Apache-2.0,
/// depending on no crate of ours.
pub(super) const OWN: &[&str] = &["freehand", "freehand-web"];

/// The repo's task runner: named as cargo expects, not published.
pub(super) const TASKS: &str = "xtask";

/// The build farm client (`tools/farm`): installed as `farm`, so named as it is run, depending on
/// no crate of ours.
pub(super) const FARM: &str = "farm";

/// A crate's name is its path: `plugin/theme/quiet` is `uses-ink-theme-quiet`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Names;

impl Names {
    /// The name `krate`'s path predicts.
    fn expected(krate: &Crate) -> String {
        let path = krate.place();
        if krate.name == TASKS || krate.name == FARM || OWN.contains(&krate.name.as_str()) {
            path.rsplit('/').next().unwrap_or(&path).to_owned()
        } else {
            // the slot directory drops
            format!("uses-ink-{}", path.strip_prefix("plugin/").unwrap_or(&path).replace('/', "-"))
        }
    }
}

impl Rule for Names {
    const NAME: &'static str = "names";

    fn check(&self, workspace: &Workspace, problems: &mut Problems) {
        for krate in &workspace.crates {
            let expected = Self::expected(krate);
            if krate.name != expected {
                problems.push(Problem::Name { krate: krate.name.clone(), path: krate.place(), expected });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_crate_is_named_for_its_path() {
        let named =
            |name, dir| Problems::of(&Names, &Workspace::sample(vec![Crate::sample(name, dir, None, &[])], &[]));
        assert!(named("uses-ink-theme-quiet", "plugin/theme/quiet").is_empty());
        assert!(named("freehand", "freehand").is_empty());
        assert!(matches!(
            named("theme-quiet", "plugin/theme/quiet").as_slice(),
            [Problem::Name { expected, .. }] if expected == "uses-ink-theme-quiet"
        ));
        assert!(matches!(
            named("freehand-pen", "freehand").as_slice(),
            [Problem::Name { expected, .. }] if expected == "uses-ink-freehand"
        ));
    }
}
