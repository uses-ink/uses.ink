//! The failure pages.

use super::{Fixtures, Pages};
use control::{Id, Refusal};
use uses_ink_web::view::Failed;
use uses_ink_web::{Cause, Failure, Field, FieldError, Frame, Screen};

impl Fixtures {
    pub(crate) fn failures() -> Vec<(&'static str, Failure)> {
        let site = Id::<control::SiteKind>::from_bytes([7; 16]);
        vec![
            ("failure_no_site", Failure::NoSite { handle: Self::handle() }),
            ("failure_signed_out", Failure::SignedOut),
            ("failure_refused", Failure::Refused { refusal: Refusal::AlreadyLive { site, deploy: Self::deploy('a') } }),
            (
                "failure_store",
                Failure::Store {
                    part: uses_ink_web::Part::Blobs,
                    source: Cause::of(&std::io::Error::other("no space left on device")),
                },
            ),
            (
                "failure_form",
                Failure::Invalid {
                    field: Field::Deploy,
                    source: FieldError::Deploy("zz".parse::<model::Hash>().unwrap_err()),
                },
            ),
        ]
    }

    pub(crate) fn failure_pages(pages: &mut Pages) {
        let handle = Self::handle();
        for (name, failure) in Self::failures() {
            pages.put(name, &Failed::new(Frame::of(&handle, Screen::Site), failure));
        }
    }
}
