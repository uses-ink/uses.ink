//! `ink:format-markdown` as a wasm component: the markdown format's native core
//! (`uses-ink-format-markdown`) behind the `ink:format` world. Built for
//! `wasm32-unknown-unknown` and wrapped with `wit-component` by `cargo xtask component`
//! (decision 0035). Nothing here decides anything: it converts and calls.

use model::{ContentPath, Read};
use sdk::bind::ink::model::read as wire;
use sdk::{Format, Served};

mod bind;

/// The format, as the component exports it.
pub(crate) struct Markdown;

impl Format for Markdown {
    const NAME: &'static str = "ink:format-markdown";

    fn handles(path: &ContentPath) -> bool {
        format_markdown::handles(path)
    }

    fn read(path: &ContentPath, bytes: &[u8]) -> Read {
        format_markdown::read(path, bytes)
    }
}

impl bind::exports::format::Guest for Markdown {
    fn handles(path: String) -> bool {
        Served::<Self>::handles(&path)
    }

    fn read(path: String, bytes: Vec<u8>) -> wire::Read {
        Served::<Self>::read(&path, &bytes)
    }
}
