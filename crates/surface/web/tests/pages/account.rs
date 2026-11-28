//! The account screen's pages.

use super::{Fixtures, Pages};
use control::SiteState;
use uses_ink_web::Document;
use uses_ink_web::view::{Account, Joined, Owned};

impl Fixtures {
    pub(crate) fn account_pages(pages: &mut Pages) {
        let handle = Self::handle();
        pages.put(
            "account",
            &Account::new(
                Self::at("2026-11-01T12:00:00Z"),
                vec![
                    Owned {
                        handle: handle.clone(),
                        state: SiteState::Active,
                        created: Self::at("2026-11-02T12:00:00Z"),
                    },
                    Owned {
                        handle: "half-gone".parse().unwrap(),
                        state: SiteState::Removing,
                        created: Self::at("2026-11-03T12:00:00Z"),
                    },
                ],
            ),
        );
        pages.put(
            "account_orgs",
            &Account::new(Self::at("2026-11-01T12:00:00Z"), vec![]).with_orgs(vec![
                Joined { name: "Garden club".to_owned(), role: control::Role::Admin, members: 4 },
                Joined { name: "<b>Press</b>".to_owned(), role: control::Role::Reader, members: 1 },
            ]),
        );
        pages.put("account_empty", &Account::new(Self::at("2026-11-01T12:00:00Z"), vec![]));
    }
}

#[test]
fn the_account_screen_lists_passkeys_with_a_remove_button() {
    use uses_ink_web::view::Kept;
    let kept = Kept {
        id: "ab".repeat(16),
        label: "Laptop".to_owned(),
        created: Fixtures::at("2026-11-27T09:30:00Z"),
        used: Some("2026-11-28".to_owned()),
        synced: true,
    };
    let account = Account::new(Fixtures::at("2026-11-01T00:00:00Z"), Vec::new()).with_passkeys(vec![kept]);
    let text = Document(&account).write().unwrap();
    assert!(
        text.contains("Laptop")
            && text.contains(r#"data-passkey="remove""#)
            && text.contains(r#"data-passkey="register""#)
    );
}

#[test]
fn the_account_screen_lists_sessions_and_ends_all_but_this_one_by_a_form() {
    use control::{Id, SessionKind};
    use uses_ink_web::view::Seen;
    let (this, other): (Id<SessionKind>, Id<SessionKind>) = (Id::from_bytes([1; 16]), Id::from_bytes([2; 16]));
    let at = Fixtures::at("2026-11-27T09:30:00Z");
    let sessions = vec![
        Seen { id: this, created: at, seen: at, current: true },
        Seen { id: other, created: at, seen: at, current: false },
    ];
    let account = Account::new(Fixtures::at("2026-11-01T00:00:00Z"), Vec::new()).with_sessions(sessions);
    let text = Document(&account).write().unwrap();
    assert!(text.contains("This session."), "{text}");
    assert!(text.contains(&format!(r#"action="/account/sessions/{other}""#)), "{text}");
    assert!(!text.contains(&format!("/account/sessions/{this}")), "the asking session has no end button");
    assert!(!text.contains("<script>"), "no script beyond the passkey one");
}

#[test]
fn the_account_screen_offers_recovery_codes_and_shows_none() {
    let account = Account::new(Fixtures::at("2026-11-01T00:00:00Z"), Vec::new()).with_recovery(7);
    let text = Document(&account).write().unwrap();
    assert!(text.contains("You have 7 unspent recovery codes."), "{text}");
    assert!(text.contains(r#"data-passkey="mint""#) && text.contains("data-recovery-codes"), "{text}");
    assert!(!text.contains("<script>"), "no script beyond the passkey one");
    let none = Account::new(Fixtures::at("2026-11-01T00:00:00Z"), Vec::new());
    assert!(!Document(&none).write().unwrap().contains("Recovery codes"), "an instance without them says nothing");
}
