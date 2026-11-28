//! A failure as a page: its stable code, the status it is sent with and the fix it words.

use std::fmt;

use control::Refusal;

use super::Failure;
use crate::Status;

/// The stable name of a failure: `noun.noun.reason`, as every code in uses.ink is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Code {
    /// `page.route.unknown`.
    RouteUnknown,
    /// `page.method.refused`.
    MethodRefused,
    /// `account.session.missing`.
    SessionMissing,
    /// `account.site.foreign`.
    SiteForeign,
    /// `site.handle.unknown`.
    HandleUnknown,
    /// `build.id.unknown`.
    BuildUnknown,
    /// `form.field.missing`.
    FieldMissing,
    /// `form.field.invalid`.
    FieldInvalid,
    /// `form.confirm.mismatch`.
    ConfirmMismatch,
    /// `control.request.refused`.
    RequestRefused,
    /// `control.store.failed`.
    StoreFailed,
    /// `build.log.cut`.
    LogCut,
    /// `page.render.failed`.
    RenderFailed,
    /// `auth.request.refused`.
    AuthRefused,
    /// `site.source.upload`.
    SourceUpload,
    /// `page.replay.unserved`.
    ReplayUnserved,
}

impl Code {
    /// The code as written.
    pub const fn name(self) -> &'static str {
        match self {
            Self::RouteUnknown => "page.route.unknown",
            Self::MethodRefused => "page.method.refused",
            Self::SessionMissing => "account.session.missing",
            Self::SiteForeign => "account.site.foreign",
            Self::HandleUnknown => "site.handle.unknown",
            Self::BuildUnknown => "build.id.unknown",
            Self::FieldMissing => "form.field.missing",
            Self::FieldInvalid => "form.field.invalid",
            Self::ConfirmMismatch => "form.confirm.mismatch",
            Self::RequestRefused => "control.request.refused",
            Self::StoreFailed => "control.store.failed",
            Self::LogCut => "build.log.cut",
            Self::RenderFailed => "page.render.failed",
            Self::AuthRefused => "auth.request.refused",
            Self::SourceUpload => "site.source.upload",
            Self::ReplayUnserved => "page.replay.unserved",
        }
    }
}

impl fmt::Display for Code {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(self.name())
    }
}

impl Failure {
    /// Its stable code.
    pub const fn code(&self) -> Code {
        match self {
            Self::Unrouted { .. } => Code::RouteUnknown,
            Self::Method { .. } => Code::MethodRefused,
            Self::SignedOut => Code::SessionMissing,
            Self::Foreign { .. } => Code::SiteForeign,
            Self::NoSite { .. } => Code::HandleUnknown,
            Self::NoBuild { .. } => Code::BuildUnknown,
            Self::Missing { .. } => Code::FieldMissing,
            Self::Invalid { .. } => Code::FieldInvalid,
            Self::Confirm { .. } => Code::ConfirmMismatch,
            Self::Refused { .. } => Code::RequestRefused,
            Self::Store { .. } => Code::StoreFailed,
            Self::Cut { .. } => Code::LogCut,
            Self::Render { .. } => Code::RenderFailed,
            Self::Auth { .. } => Code::AuthRefused,
            Self::Upload { .. } => Code::SourceUpload,
            Self::NoReplay => Code::ReplayUnserved,
        }
    }

    /// The status its page is sent with.
    pub const fn status(&self) -> Status {
        match self {
            Self::Unrouted { .. } | Self::NoSite { .. } | Self::NoBuild { .. } | Self::NoReplay => Status::NotFound,
            Self::Auth { status, .. } => *status,
            Self::Method { .. } => Status::MethodNotAllowed,
            Self::SignedOut => Status::Unauthorized,
            Self::Foreign { .. } => Status::Forbidden,
            Self::Missing { .. } | Self::Invalid { .. } | Self::Confirm { .. } => Status::BadRequest,
            Self::Refused { .. } | Self::Upload { .. } => Status::Conflict,
            Self::Store { .. } | Self::Cut { .. } | Self::Render { .. } => Status::Internal,
        }
    }

    /// What to do about it, in a sentence of its own.
    pub fn fix(&self) -> Fix<'_> {
        Fix(self)
    }
}

/// The fix for a failure, worded.
#[derive(Clone, Copy, Debug)]
pub struct Fix<'a>(&'a Failure);

impl fmt::Display for Fix<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Failure::Unrouted { .. } => out.write_str("use the links on the page; the five screens are Site, What we understood, Deploys, Settings and Account"),
            Failure::Method { .. } => out.write_str("open the page by its link, or send the form from the page it belongs to"),
            Failure::SignedOut => out.write_str("sign in again; your session has ended or never began"),
            Failure::Foreign { .. } => out.write_str("sign in with the account that owns the site"),
            Failure::NoSite { .. } => out.write_str("check the handle in the address; your own site is listed under Account"),
            Failure::NoBuild { .. } => out.write_str("open the build from the Deploys screen"),
            Failure::Missing { .. } | Failure::Invalid { .. } => {
                out.write_str("send the form from its page, without changing its fields")
            }
            Failure::Confirm { .. } => out.write_str("type the site's handle exactly, or leave the site as it is"),
            Failure::Refused { refusal } => Refused(refusal).fmt(out),
            Failure::Store { .. } => out.write_str("try again in a minute; if it fails again, give the operator this code and the cause below"),
            Failure::Cut { .. } => out.write_str("reload the page: a finished build shows its whole log"),
            Failure::Render { .. } => out.write_str("reload the page; if it fails again, give the operator this code and the cause below"),
            Failure::Auth { status: Status::TooMany, .. } => out.write_str("wait a while, then ask again: the server limits how often a link may be asked for"),
            Failure::Auth { status: Status::Forbidden, .. } => out.write_str("open this page from the address the server was started with, not from another site"),
            Failure::Auth { status: Status::NotFound, .. } => out.write_str("the link was used, has ended or never was: ask for a new one on the sign-in page"),
            Failure::Auth { .. } => out.write_str("check what you typed and try again; if it repeats, give the operator this code and the cause below"),
            Failure::Upload { .. } => out.write_str("change the folder and run `ink push` again"),
            Failure::NoReplay => out.write_str("run `ink explain` in the site's folder, which holds the source this page would replay"),
        }
    }
}

/// What to do about each thing the control plane refuses.
#[derive(Clone, Copy, Debug)]
struct Refused<'a>(&'a Refusal);

impl fmt::Display for Refused<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self.0 {
            Refusal::Reserved { .. } | Refusal::Taken { .. } | Refusal::Held { .. } => "choose another handle",
            Refusal::AlreadyLive { .. } => "nothing to do: this deploy is the one being served",
            Refusal::NoDeploy { .. } | Refusal::NotLive { .. } => "open Deploys and pick a deploy the site has",
            Refusal::NoSite { .. } | Refusal::Removing { .. } => {
                "the site is gone or going; open Account to see your sites"
            }
            Refusal::MissingBlob { .. } | Refusal::NoManifest { .. } => {
                "build the site again; its stored files are incomplete"
            }
            _ => "reload the page and try again; if it repeats, give the operator this code",
        })
    }
}
