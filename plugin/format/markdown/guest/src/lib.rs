//! `ink:format-markdown` as a wasm component: the markdown format's native core
//! (`uses-ink-format-markdown`) behind the `ink:format` world. Built for
//! `wasm32-unknown-unknown` and wrapped with `wit-component` by `cargo xtask component`
//! (decision 0035). Nothing here decides anything: it converts and calls.

use model::{ContentPath, Diagnostic, Document, Front, Problem, Read};
use sdk::bind::ink::model::read as wire;

mod bind;

/// The format, as the component exports it.
pub(crate) struct Markdown;

impl bind::exports::format::Guest for Markdown {
    fn handles(path: String) -> bool {
        path.parse::<ContentPath>().is_ok_and(|path| format_markdown::handles(&path))
    }

    /// The file read; a path the model refuses, or a read the boundary cannot carry, is an empty
    /// read with the diagnostic saying why.
    fn read(path: String, bytes: Vec<u8>) -> wire::Read {
        let read = match path.parse::<ContentPath>() {
            Ok(path) => format_markdown::read(&path, &bytes),
            Err(refused) => Self::refused(Problem::PluginValueMalformed {
                plugin: Self::NAME.to_owned(),
                reason: refused.to_string(),
            }),
        };
        wire::Read::try_from(&read).unwrap_or_else(|unmirrored| {
            let problem = Problem::PluginValueUnmirrored { reason: unmirrored.to_string() };
            let empty = wire::Read::try_from(&Self::refused(problem.clone()));
            empty.unwrap_or_else(|_| wire::Read {
                document: wire::Document {
                    blocks: Vec::new(),
                    inlines: Vec::new(),
                    body: Vec::new(),
                    footnotes: Vec::new(),
                },
                front: wire::Front {
                    title: None,
                    date: None,
                    updated: None,
                    slug: None,
                    tags: None,
                    publish: None,
                    draft: None,
                    kind: None,
                    description: None,
                    language: None,
                    block: None,
                },
                heading: None,
                diagnostics: vec![sdk::Host::carried(&Diagnostic::new(problem))],
                notices: Vec::new(),
            })
        })
    }
}

impl Markdown {
    const NAME: &'static str = "ink:format-markdown";

    /// An empty read holding only `problem`.
    fn refused(problem: Problem) -> Read {
        let mut read = Read::new(Document::default(), Front::default(), None);
        read.diagnostics.push(Diagnostic::new(problem));
        read
    }
}
