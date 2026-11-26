//! Which defaults each code announces, and with what. How a code's values read is the model's
//! [`Word`], shared with config and `ink explain`.

use model::code::{
    Authors, Date, Description, Home, Ignored, Kind, Language, Layout, Nav, Publish, Root, SiteTitle, Slug, Summary,
    Tags, Title, Updated,
};
use model::decision::Decision;
use model::{Announcement, DefaultReason, Evidence, Word, Worded};

/// A code that says what taking its default means.
pub(crate) trait Explain: Word {
    /// The announcement taking `value` by default, for `reason`, calls for. Defaults that only
    /// mean "nothing to do" (nothing ignored, no tags, never updated, a public page published)
    /// announce nothing, or every file would; this is the default.
    fn announce(_reason: DefaultReason, _value: &Self::Value) -> Option<Announcement> {
        None
    }

    /// The announcement `decision` calls for, when it took a default.
    fn notice(decision: &Decision<Self>) -> Option<Announcement> {
        let Evidence::Default { reason } = decision.evidence else { return None };
        Self::announce(reason, &decision.value)
    }

    /// `notice.default.chosen` for `value`.
    fn chosen(value: &Self::Value) -> Announcement {
        Announcement::NoticeDefaultChosen { decision: Self::CODE, value: Worded::<Self>::new(value).to_string() }
    }
}

/// Implements [`Explain`] for codes whose default is always announced. The same impl over a list of types, which a generic cannot write.
///
/// ```text
/// announced!(Root, Layout);
/// // makes `impl Explain for Root` announcing `notice.default.chosen` on every default
/// ```
macro_rules! announced {
    ($($code:ty),+) => {
        $(
            impl Explain for $code {
                fn announce(_reason: DefaultReason, value: &Self::Value) -> Option<Announcement> {
                    Some(Self::chosen(value))
                }
            }
        )+
    };
}

announced!(Root, Layout, Kind, Slug, SiteTitle, Language, Nav, Authors);

impl Explain for Title {}

impl Explain for Summary {}

impl Explain for Description {}

impl Explain for Ignored {}

impl Explain for Publish {
    /// Only a vault's or a private source's default is worth saying: nothing is published until
    /// it is selected.
    fn announce(reason: DefaultReason, value: &bool) -> Option<Announcement> {
        (reason == DefaultReason::NotSelected).then(|| Self::chosen(value))
    }
}

impl Explain for Date {}

impl Explain for Updated {}

impl Explain for Tags {}

impl Explain for Home {
    /// A generated home is always said.
    fn announce(_reason: DefaultReason, _value: &Option<model::ContentPath>) -> Option<Announcement> {
        Some(Announcement::SiteHomeGenerated)
    }
}
