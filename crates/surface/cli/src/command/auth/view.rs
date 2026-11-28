//! The results of the key commands as typed blocks of the terminal layer.

use std::fmt;

use jiff::Timestamp;

use super::wire::{Enrolled, Row, Whoami};
use crate::output::term::{Doc, Empty, Line, Listed, Record, Render, Role};

/// A moment, UTC and ISO, as tables show it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Moment(pub(crate) Timestamp);

impl fmt::Display for Moment {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{}", self.0.strftime("%Y-%m-%d %H:%MZ"))
    }
}

/// What a person is asked to do: the code to type and the fingerprint to compare, on the other
/// screen.
#[derive(Debug)]
pub(crate) struct Asking<'a> {
    pub(crate) ask: &'a Enrolled,
    pub(crate) label: &'a str,
    pub(crate) api: &'a str,
}

impl Render for Asking<'_> {
    fn render(&self, into: &mut Doc) {
        Record::new(0)
            .field("code", Line::new().with(Role::Id, self.ask.user_code.clone()))
            .field("fingerprint", Line::new().with(Role::Id, self.ask.fingerprint.clone()))
            .field("label", Line::plain(self.label))
            .field("ends", Line::plain(Moment(self.ask.expires).to_string()))
            .render(into);
        into.blank();
        into.push(Line::plain(format!(
            "approve it as a signed-in person at {}/v1/approve/{}, after checking the fingerprint matches",
            self.api, self.ask.user_code
        )));
    }
}

/// A key that is now enrolled.
#[derive(Debug)]
pub(crate) struct Joined<'a> {
    pub(crate) fingerprint: &'a str,
    pub(crate) label: &'a str,
    pub(crate) kind: &'a str,
    pub(crate) expires: Option<Timestamp>,
    pub(crate) kept: Option<&'a str>,
}

impl Render for Joined<'_> {
    fn render(&self, into: &mut Doc) {
        let mut fields = Record::new(0)
            .field("key", Line::new().with(Role::Good, "enrolled"))
            .field("fingerprint", Line::new().with(Role::Id, self.fingerprint))
            .field("kind", Line::plain(self.kind))
            .field("label", Line::plain(self.label));
        if let Some(expires) = self.expires {
            fields = fields.field("ends", Line::plain(Moment(expires).to_string()));
        }
        if let Some(kept) = self.kept {
            fields = fields.field("kept", Line::plain(kept));
        }
        fields.render(into);
    }
}

/// Who the key is.
#[derive(Debug)]
pub(crate) struct Who<'a>(pub(crate) &'a Whoami);

impl Render for Who<'_> {
    fn render(&self, into: &mut Doc) {
        let who = self.0;
        let rights = who
            .grants
            .iter()
            .map(|grant| format!("{} on {}", grant.rights.join(", "), grant.scope))
            .collect::<Vec<_>>()
            .join("; ");
        let mut fields = Record::new(0)
            .field("account", Line::new().with(Role::Id, who.account.clone()))
            .field("key", Line::plain(format!("{} ({})", who.label, who.kind)));
        if let Some(print) = &who.fingerprint {
            fields = fields.field("fingerprint", Line::new().with(Role::Id, print.clone()));
        }
        fields = fields.field("may", Line::plain(rights));
        if let Some(expires) = who.expires {
            fields = fields.field("ends", Line::plain(Moment(expires).to_string()));
        }
        fields.render(into);
    }
}

/// What the list of keys says when the account has none to show.
const EMPTY: Empty = Empty {
    said: "No keys to list.",
    next: Some(("ink key add --label ci --out ci.key", "Make a key for another machine, such as a CI job")),
};

/// The keys of an account: one row each, with the day each was last used and nothing finer.
#[derive(Debug)]
pub(crate) struct Keys<'a>(pub(crate) &'a [Row]);

impl Render for Keys<'_> {
    fn render(&self, into: &mut Doc) {
        let mut table = Listed::new(EMPTY, &["fingerprint", "kind", "label", "rights", "ends", "last used", "state"]);
        for row in self.0 {
            let (role, state) = match &row.revoked {
                Some(revoked) => (Role::Warn, format!("revoked ({})", revoked.why)),
                None => (Role::Good, "live".to_owned()),
            };
            table = table.row(vec![
                Line::new().with(Role::Id, row.fingerprint.clone()),
                Line::plain(row.kind.clone()),
                Line::plain(row.label.clone()),
                Line::plain(format!("{} on {}", row.grant.rights.join(","), row.grant.scope)),
                Line::plain(Moment(row.expires).to_string()),
                Line::plain(row.last_used.clone().unwrap_or_else(|| "never".to_owned())),
                Line::new().with(role, state),
            ]);
        }
        table.render(into);
    }
}

/// A key ended.
#[derive(Debug)]
pub(crate) struct Ended<'a>(pub(crate) &'a Row);

impl Render for Ended<'_> {
    fn render(&self, into: &mut Doc) {
        let why = self.0.revoked.as_ref().map_or("revoked", |revoked| revoked.why.as_str());
        Record::new(0)
            .field("key", Line::new().with(Role::Warn, why))
            .field("fingerprint", Line::new().with(Role::Id, self.0.fingerprint.clone()))
            .field("label", Line::plain(self.0.label.clone()))
            .render(into);
    }
}
