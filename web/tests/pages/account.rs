//! The account screen's pages.

use super::{Fixtures, Pages};
use control::SiteState;
use uses_ink_web::Document;
use uses_ink_web::view::{Account, Owned};

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
