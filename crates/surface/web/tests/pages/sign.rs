//! The pages around signing in and the stored log of a build.

use super::{Fixtures, Pages};
use control::Handle;
use uses_ink_web::Token;
use uses_ink_web::view::{Ended, Entering, Outcome, Recorded, Sent, SignIn, Stored};

impl Fixtures {
    pub(crate) fn sign_pages(pages: &mut Pages) {
        pages.put("signin", &SignIn::new());
        pages.put("sent", &Sent::new());
        pages.put("entering", &Entering::new(Token::parse(&"ab".repeat(32)).unwrap()));
        let handle: Handle = Self::handle();
        let lines = vec![
            Recorded { event: "started".to_owned(), detail: "trigger manual".to_owned(), bad: false },
            Recorded { event: "refused".to_owned(), detail: "config.syntax.invalid".to_owned(), bad: true },
        ];
        let outcome =
            Outcome { status: build::Status::Refused, deploy: None, wall: std::time::Duration::from_millis(40) };
        pages.put(
            "stored_done",
            &Stored::new(handle.clone(), Self::build(1), lines.clone(), Some(Ended::Finished(outcome))),
        );
        pages.put("stored_running", &Stored::new(handle, Self::build(2), Vec::new(), None));
    }
}

#[test]
fn a_token_is_a_segment_of_the_safe_alphabet_and_no_longer_than_its_limit() {
    assert!(Token::parse(&"a".repeat(Token::LONGEST)).is_some());
    assert!(Token::parse(&"a".repeat(Token::LONGEST + 1)).is_none());
    for bad in ["", "a/b", "a b", "a.b", "a?b", "\u{e9}"] {
        assert!(Token::parse(bad).is_none(), "{bad:?}");
    }
}
