//! The rule over layers: a crate under `crates/<layer>/` depends only on crates of its own layer
//! or an earlier one (docs/layout.md section 3.2), so the layer list is checked against the
//! dependencies and not trusted.

use crate::layout::model::Workspace;
use crate::layout::{Problem, Problems, Reach, Rule};

/// A layer depends only on the layers before it; an open layer on itself alone.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Layer;

impl Rule for Layer {
    const NAME: &'static str = "layer";

    fn check(&self, workspace: &Workspace, problems: &mut Problems) {
        let layers = &workspace.config.layers;
        let placed: Vec<(&str, String)> = workspace
            .crates
            .iter()
            .filter_map(|krate| krate.layer(&layers.order).map(|layer| (krate.name.as_str(), layer)))
            .collect();
        for krate in &workspace.crates {
            let Some(from) = krate.layer(&layers.order) else { continue };
            for dependency in krate.dependencies.iter().filter(|dependency| dependency.inside) {
                let Some((_, to)) = placed.iter().find(|(name, _)| *name == dependency.name) else { continue };
                if !layers.allows(&from, to) {
                    problems.push(Problem::Layer(Reach {
                        krate: krate.name.clone(),
                        on: dependency.name.clone(),
                        from: from.clone(),
                        to: to.clone(),
                    }));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::model::Crate;

    fn found(crates: Vec<Crate>) -> Vec<Problem> {
        let mut workspace = Workspace::sample(crates, &[]);
        workspace.config.layers.order = ["core", "port", "standalone"].map(str::to_owned).to_vec();
        workspace.config.layers.open = vec!["standalone".to_owned()];
        Problems::of(&Layer, &workspace)
    }

    #[test]
    fn a_crate_may_depend_on_its_layer_an_earlier_one_and_an_open_one() {
        let crates = vec![
            Crate::sample("uses-ink-model", "crates/core/model", None, &[("freehand", None)]),
            Crate::sample("uses-ink-blob", "crates/port/blob", None, &[("uses-ink-model", None)]),
            Crate::sample("uses-ink-db", "crates/port/db", None, &[("uses-ink-blob", None)]),
            Crate::sample("freehand", "crates/standalone/freehand", None, &[]),
        ];
        assert!(found(crates).is_empty());
    }

    #[test]
    fn a_crate_may_not_depend_on_a_later_layer() {
        let crates = vec![
            Crate::sample("uses-ink-model", "crates/core/model", None, &[("uses-ink-blob", None)]),
            Crate::sample("uses-ink-blob", "crates/port/blob", None, &[]),
        ];
        assert!(matches!(
            found(crates).as_slice(),
            [Problem::Layer(Reach { krate, on, from, to })]
                if krate == "uses-ink-model" && on == "uses-ink-blob" && from == "core" && to == "port"
        ));
    }

    #[test]
    fn an_open_crate_depends_on_its_own_layer_alone() {
        let crates = vec![
            Crate::sample("freehand", "crates/standalone/freehand", None, &[("uses-ink-model", None)]),
            Crate::sample("freehand-web", "crates/standalone/freehand-web", None, &[("freehand", None)]),
            Crate::sample("uses-ink-model", "crates/core/model", None, &[]),
        ];
        assert!(matches!(found(crates).as_slice(), [Problem::Layer(Reach { krate, .. })] if krate == "freehand"));
    }

    #[test]
    fn a_crate_outside_the_layers_is_left_to_the_other_rules() {
        let crates = vec![
            Crate::sample("xtask", "tools/xtask", None, &[("uses-ink-model", None)]),
            Crate::sample("uses-ink-model", "crates/core/model", None, &[("xtask", None)]),
        ];
        assert!(found(crates).is_empty());
    }
}
