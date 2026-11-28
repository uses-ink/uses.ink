//! What `ink` says when it panics: a bug, with where it happened and who to tell (`docs/cli.md`
//! section 9). The code is not in the catalog on purpose: it is not part of the documented set.

use std::fmt;
use std::io::Write as _;
use std::panic::PanicHookInfo;

/// A panic as the person sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Panicked {
    location: String,
    message: String,
}

impl Panicked {
    /// The code the report carries.
    pub const CODE: &'static str = "cli.panic";

    /// Where bugs are reported.
    const REPORT: &'static str = "https://github.com/uses-ink/uses.ink/issues";

    /// A panic at `location` that said `message`.
    pub fn new(location: impl Into<String>, message: impl Into<String>) -> Self {
        Self { location: location.into(), message: message.into() }
    }

    /// The panic `info` describes.
    pub fn of(info: &PanicHookInfo<'_>) -> Self {
        let payload = info.payload();
        let message = payload
            .downcast_ref::<&str>()
            .map(|text| (*text).to_owned())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "it gave no message".to_owned());
        let location =
            info.location().map_or_else(|| "an unknown place".to_owned(), |at| format!("{}:{}", at.file(), at.line()));
        Self { location, message }
    }

    /// Makes every panic of the process print as `error[cli.panic]` on stderr. The process then
    /// ends as Rust ends on a panic, with exit code 101.
    pub fn install() {
        std::panic::set_hook(Box::new(|info| {
            // a stderr that cannot be written has nowhere left to say so
            let _unsaid = writeln!(std::io::stderr(), "{}", Self::of(info));
        }));
    }
}

impl fmt::Display for Panicked {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            out,
            "error[{}]: ink stopped on a bug of its own, at {}: {}\n  fix: report it at {}, with the command you ran",
            Self::CODE,
            self.location,
            self.message,
            Self::REPORT
        )
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn a_panic_prints_its_code_its_place_and_where_to_report_it() {
        let said = Panicked::new("src/lib.rs:7", "index out of bounds").to_string();
        assert!(said.starts_with("error[cli.panic]: "), "{said}");
        assert!(said.contains("src/lib.rs:7") && said.contains("index out of bounds"), "{said}");
        assert!(said.contains("fix: report it at https://"), "{said}");
    }
}
