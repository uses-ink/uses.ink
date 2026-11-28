//! The rule over licences: each crate's matches its kind (decision 0026).

use super::names::OWN;
use crate::layout::model::{Kind, Workspace};
use crate::layout::{Problem, Problems, Rule};

/// A crate's licence matches its kind: permissive for a crate with a name of its own and for
/// what a third-party plugin links, the service's otherwise (decision 0026). A permissive crate
/// depends on no crate of ours under the service's licence, so linking it never pulls one in.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Licence;

impl Licence {
    pub(crate) const PERMISSIVE: &'static str = "MIT OR Apache-2.0";
    const SERVICE: &'static str = "AGPL-3.0-or-later";

    /// Crates of ours a third-party plugin links: the model it speaks and the SDK.
    const LINKED: &'static [&'static str] = &["uses-ink-model", "uses-ink-sdk"];

    /// Whether the crate named `name` must be permissive.
    fn is_permissive(name: &str) -> bool {
        OWN.contains(&name) || Self::LINKED.contains(&name)
    }
}

impl Rule for Licence {
    const NAME: &'static str = "licence";

    fn check(&self, workspace: &Workspace, problems: &mut Problems) {
        let ours: Vec<&str> = workspace.crates.iter().map(|c| c.name.as_str()).collect();
        for krate in &workspace.crates {
            let permissive = Self::is_permissive(&krate.name);
            let expected = if permissive { Self::PERMISSIVE } else { Self::SERVICE };
            if krate.licence.as_deref() != Some(expected) {
                problems.push(Problem::Licence { krate: krate.name.clone(), found: krate.licence.clone(), expected });
            }
            if !permissive {
                continue;
            }
            // a dev-dependency is not linked by whoever uses the crate
            for dependency in krate.dependencies.iter().filter(|d| d.kind != Some(Kind::Dev)) {
                let name = dependency.name.as_str();
                if dependency.inside && ours.contains(&name) && !Self::is_permissive(name) {
                    problems.push(Problem::Copyleft { krate: krate.name.clone(), on: dependency.name.clone() });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::model::Crate;

    #[test]
    fn a_licence_is_the_one_its_kind_calls_for() {
        let check = |krate| Problems::of(&Licence, &Workspace::sample(vec![krate], &[]));
        let model = |licence| Crate::sample("uses-ink-model", "model", licence, &[]);
        assert!(check(model(Some(Licence::PERMISSIVE))).is_empty());
        assert!(matches!(
            check(model(Some(Licence::SERVICE))).as_slice(),
            [Problem::Licence { expected, .. }] if *expected == Licence::PERMISSIVE
        ));
        let engine = Crate::sample("uses-ink-engine", "engine", None, &[]);
        assert!(matches!(
            check(engine).as_slice(),
            [Problem::Licence { found: None, expected, .. }] if *expected == Licence::SERVICE
        ));
    }

    #[test]
    fn a_permissive_crate_may_not_link_the_service() {
        let engine = || Crate::sample("uses-ink-engine", "engine", Some(Licence::SERVICE), &[]);
        let linked = Crate::sample("uses-ink-model", "model", Some(Licence::PERMISSIVE), &[("uses-ink-engine", None)]);
        let found = Problems::of(&Licence, &Workspace::sample(vec![linked, engine()], &[]));
        assert!(matches!(
            found.as_slice(),
            [Problem::Copyleft { krate, on }] if krate == "uses-ink-model" && on == "uses-ink-engine"
        ));
        // a dev-dependency is not linked by whoever uses the crate
        let tested = Crate::sample(
            "uses-ink-model",
            "model",
            Some(Licence::PERMISSIVE),
            &[("uses-ink-engine", Some(Kind::Dev))],
        );
        assert!(Problems::of(&Licence, &Workspace::sample(vec![tested, engine()], &[])).is_empty());
    }
}
