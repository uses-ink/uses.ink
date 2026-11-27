//! A build compared with its baseline (decision 0042): what became public, what is no longer
//! served or moved, and each decision that changed with the first cause that holds, in order:
//! config named by either side's evidence, a format plugin that is another build, an ability
//! a history-based decision reads, the subject's own file, `ink.toml`, any other file, else ink
//! itself, which is a warning.

use std::collections::{BTreeMap, BTreeSet};

use model::record::{Against, Build, Cause, Change, Changes, Inputs, Kept, Moved, Routed, Touched};
use model::{
    Ability, AnyCode, AnyDecision, ContentPath, DefaultReason, Diagnostic, Evidence, Hash, Problem, Route, Snapshot,
    Subject,
};

use crate::Understood;

/// The release of ink this engine is: informative in a record, since the rules decide.
pub const RELEASE: &str = env!("CARGO_PKG_VERSION");

impl Understood {
    /// The record of this build of `snapshot`, made from `inputs`: every decision, every file
    /// and the route of every published file.
    pub fn record(&self, snapshot: &Snapshot, inputs: Inputs) -> Build {
        let files = snapshot.tree().iter().map(|entry| Kept::new(entry.path.clone(), entry.blob)).collect();
        let routes = self.site.as_ref().map_or_else(Vec::new, |site| {
            let pages = site.pages().iter().map(|page| Routed::new(page.file.clone(), page.route.clone()));
            let assets = site.assets().iter().map(|asset| Routed::new(asset.file.clone(), asset.route.clone()));
            pages.chain(assets).collect()
        });
        Build::new(inputs, self.decisions.clone(), files, routes)
    }
}

/// A build's changes against its baseline, and the warnings they call for.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Compared {
    /// The change set.
    pub changes: Changes,
    /// `engine.decision.unstable` for each change ink itself caused.
    pub diagnostics: Vec<Diagnostic>,
}

impl Compared {
    /// `after` compared with `before`, the record kept by the build `against` names. With no
    /// earlier build every published file is newly public and no decision is listed, as all are
    /// new; with an earlier build that kept no record, nothing can be said.
    pub fn of(against: Against, before: Option<&Build>, after: &Build) -> Self {
        let mut changes = Changes::new(against);
        let was: BTreeMap<&ContentPath, &Route> =
            before.iter().flat_map(|build| &build.routes).map(|routed| (&routed.path, &routed.route)).collect();
        let is: BTreeMap<&ContentPath, &Route> =
            after.routes.iter().map(|routed| (&routed.path, &routed.route)).collect();
        let known = before.is_some() || changes.against == Against::Nothing;
        for routed in after.routes.iter().filter(|_| known) {
            match was.get(&routed.path) {
                None => changes.public.push(routed.clone()),
                Some(&route) if *route != routed.route => {
                    changes.moved.push(Moved::new(routed.path.clone(), route.clone(), routed.route.clone()));
                }
                Some(_) => {}
            }
        }
        let gone = before.iter().flat_map(|build| &build.routes).filter(|routed| !is.contains_key(&routed.path));
        changes.withdrawn.extend(gone.cloned());
        let mut diagnostics = Vec::new();
        if let Some(before) = before {
            changes.decisions = Attribution::new(before, after).changes();
            for change in changes.decisions.iter().filter(|change| change.cause == Cause::Engine) {
                let problem = Problem::EngineDecisionUnstable { decision: change.code };
                diagnostics.push(Diagnostic::new(problem).on(change.subject.clone()));
            }
        }
        Self { changes, diagnostics }
    }
}

/// What two records hold, indexed for telling a change's cause.
#[derive(Debug)]
struct Attribution<'a> {
    before: &'a Build,
    after: &'a Build,
    files: [BTreeMap<&'a ContentPath, Hash>; 2],
}

/// A decision's code and subject: what pairs a decision of one build with the other's.
type Key = (AnyCode, Subject);

impl<'a> Attribution<'a> {
    fn new(before: &'a Build, after: &'a Build) -> Self {
        let index = |build: &'a Build| build.files.iter().map(|kept| (&kept.path, kept.blob)).collect();
        Self { before, after, files: [index(before), index(after)] }
    }

    /// Every decision that differs, in the order the later build made them, then the ones it no
    /// longer makes.
    fn changes(&self) -> Vec<Change> {
        let index = |build: &'a Build| -> BTreeMap<Key, &'a AnyDecision> {
            build.decisions.iter().map(|decision| ((decision.code(), decision.subject()), decision)).collect()
        };
        let (was, is) = (index(self.before), index(self.after));
        let mut changes = Vec::new();
        for decision in &self.after.decisions {
            let key = (decision.code(), decision.subject());
            let before = was.get(&key).copied();
            if before.is_none_or(|before| !before.agrees(decision)) {
                let cause = self.cause(&key, before, Some(decision));
                changes.push(Change::new(key, before.cloned(), Some(decision.clone()), cause));
            }
        }
        for decision in &self.before.decisions {
            let key = (decision.code(), decision.subject());
            if !is.contains_key(&key) {
                let cause = self.cause(&key, Some(decision), None);
                changes.push(Change::new(key, Some(decision.clone()), None, cause));
            }
        }
        changes
    }

    /// The first cause that holds for `key` going from `before` to `after`.
    fn cause(&self, key: &Key, before: Option<&AnyDecision>, after: Option<&AnyDecision>) -> Cause {
        let evidence: Vec<&Evidence> = before.into_iter().chain(after).map(AnyDecision::evidence).collect();
        let (was, is) = (&self.before.inputs, &self.after.inputs);
        if evidence.iter().any(|evidence| matches!(evidence, Evidence::Config { .. })) {
            return Cause::Config;
        }
        if let Some(plugin) = self.plugin(key) {
            return Cause::Plugin { plugin };
        }
        let historic = evidence.iter().any(|evidence| {
            matches!(evidence, Evidence::History { .. } | Evidence::Default { reason: DefaultReason::OnlyBound })
        });
        if let Some(ability) = self.ability().filter(|_| historic) {
            return Cause::Ability { ability };
        }
        if let Some(touched) = self.touched(&key.1) {
            return Cause::Content(touched);
        }
        if was.config != is.config {
            return Cause::Config;
        }
        if self.files[0] != self.files[1] {
            return Cause::Content(Touched::Elsewhere);
        }
        Cause::Engine
    }

    /// A format plugin that is another build of itself, or ran in only one build, when the
    /// subject is a file.
    fn plugin(&self, key: &Key) -> Option<model::record::PluginId> {
        let Subject::File(_) = key.1 else { return None };
        let ran = |inputs: &'a Inputs| -> BTreeSet<&'a model::record::Plugin> { inputs.plugins.iter().collect() };
        let (was, is) = (ran(&self.before.inputs), ran(&self.after.inputs));
        was.symmetric_difference(&is).map(|plugin| &plugin.id).find(|id| id.is_format()).cloned()
    }

    /// An ability one build's source had, or lacked in part, and the other's did not.
    fn ability(&self) -> Option<Ability> {
        let had = |inputs: &'a Inputs| -> BTreeSet<Ability> { inputs.abilities.iter().copied().collect() };
        let lacked = |inputs: &'a Inputs| -> BTreeSet<Ability> { inputs.lacking.iter().copied().collect() };
        let (was, is) = (&self.before.inputs, &self.after.inputs);
        let gained = had(was).symmetric_difference(&had(is)).next().copied();
        gained.or_else(|| lacked(was).symmetric_difference(&lacked(is)).next().copied())
    }

    /// How the subject's own file changed, when it did.
    fn touched(&self, subject: &Subject) -> Option<Touched> {
        let Subject::File(path) = subject else { return None };
        match (self.files[0].get(path), self.files[1].get(path)) {
            (None, Some(_)) => Some(Touched::Added),
            (Some(_), None) => Some(Touched::Removed),
            (Some(was), Some(is)) if was != is => Some(Touched::Edited),
            _ => None,
        }
    }
}
