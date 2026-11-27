//! What a refusal of the key store is answered with, once, for the API and the door.

use control::keys::Refused;

use crate::reply::Status;

/// A refusal of the key store, with its catalog code and its status.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Turned<'a>(pub &'a Refused);

impl Turned<'_> {
    /// The catalog code.
    pub(crate) const fn code(self) -> &'static str {
        match self.0 {
            Refused::NoEnrolment => "auth.enrolment.expired",
            Refused::Fingerprint => "auth.enrolment.mismatch",
            Refused::NoKey => "auth.key.unknown",
            Refused::Site | Refused::Kind { .. } => "auth.enrolment.site",
            Refused::Rights => "auth.enrolment.rights",
            Refused::Lifetime { .. } => "auth.enrolment.lifetime",
            Refused::CodeTaken | Refused::KeyTaken => "auth.enrolment.taken",
            _ => "auth.enrolment.refused",
        }
    }

    /// The status.
    pub(crate) const fn status(self) -> Status {
        match self.0 {
            Refused::NoEnrolment | Refused::NoKey => Status::NotFound,
            Refused::Fingerprint | Refused::CodeTaken | Refused::KeyTaken => Status::Conflict,
            _ => Status::Unprocessable,
        }
    }
}
