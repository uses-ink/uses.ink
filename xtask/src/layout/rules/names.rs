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
    /// `path` without the `crates/<layer>/` it may sit under, for a layer of `layers`.
    fn below<'p>(path: &'p str, layers: &[String]) -> &'p str {
        let Some((layer, below)) = path.strip_prefix("crates/").and_then(|rest| rest.split_once('/')) else {
            return path;
        };
        if layers.iter().any(|known| known == layer) { below } else { path }
    }

    /// The name `krate`'s path predicts.
    fn expected(krate: &Crate, layers: &[String]) -> String {
        let place = krate.place();
        let path = Self::below(&place, layers);
        if krate.name == TASKS || krate.name == FARM || OWN.contains(&krate.name.as_str()) {
            path.rsplit('/').next().unwrap_or(path).to_owned()
        } else {
            // the slot directory drops
            format!("uses-ink-{}", path.strip_prefix("plugin/").unwrap_or(path).replace('/', "-"))
        }
    }
}

impl Rule for Names {
    const NAME: &'static str = "names";

    fn check(&self, workspace: &Workspace, problems: &mut Problems) {
        for krate in &workspace.crates {
            let expected = Self::expected(krate, &workspace.config.layers.order);
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

    #[test]
    fn a_crate_below_a_layer_is_named_for_the_path_below_it() {
        let workspace = |name, dir| {
            let mut workspace = Workspace::sample(vec![Crate::sample(name, dir, None, &[])], &[]);
            workspace.config.layers.order = vec!["core".to_owned(), "runtime".to_owned(), "standalone".to_owned()];
            Problems::of(&Names, &workspace)
        };
        assert!(workspace("uses-ink-model", "crates/core/model").is_empty());
        assert!(workspace("uses-ink-theme-quiet", "crates/runtime/plugin/theme/quiet").is_empty());
        assert!(workspace("freehand", "crates/standalone/freehand").is_empty());
        assert!(matches!(
            workspace("uses-ink-model", "crates/other/model").as_slice(),
            [Problem::Name { expected, .. }] if expected == "uses-ink-crates-other-model"
        ));
    }
}
