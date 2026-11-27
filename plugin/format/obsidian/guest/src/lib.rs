//! `ink:format-obsidian` as a wasm component: the obsidian format's native core
//! (`uses-ink-format-obsidian`) behind the `ink:format` world. Built for
//! `wasm32-unknown-unknown` and wrapped with `wit-component` by `cargo xtask component`
//! (decision 0035). Nothing here decides anything: it converts and calls.

use model::{ContentPath, Read};
use sdk::bind::ink::model::read as wire;
use sdk::{Format, Served};

mod bind;

/// The format, as the component exports it.
pub(crate) struct Obsidian;

impl Format for Obsidian {
    const NAME: &'static str = "ink:format-obsidian";

    fn handles(path: &ContentPath) -> bool {
        format_obsidian::handles(path)
    }

    fn read(path: &ContentPath, bytes: &[u8]) -> Read {
        format_obsidian::read(path, bytes)
    }
}

impl bind::exports::format::Guest for Obsidian {
    fn handles(path: String) -> bool {
        Served::<Self>::handles(&path)
    }

    fn read(path: String, bytes: Vec<u8>) -> wire::Read {
        Served::<Self>::read(&path, &bytes)
    }
}
