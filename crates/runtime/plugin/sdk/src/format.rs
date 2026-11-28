//! A format component's exports, served from its native core: the path parsed, the file read,
//! and the read carried across the boundary, every refusal a diagnostic in the read.

use std::marker::PhantomData;

use model::{ContentPath, Diagnostic, Document, Front, Problem, Read};

use crate::Host;
use crate::bind::ink::model::read as wire;

/// A format's native core, as a component serves it.
pub trait Format {
    /// The plugin's id, which a refusal names (`ink:format-markdown`).
    const NAME: &'static str;

    /// Whether the format reads the file at `path`.
    fn handles(path: &ContentPath) -> bool;

    /// The file at `path`, read.
    fn read(path: &ContentPath, bytes: &[u8]) -> Read;
}

/// The `ink:format` exports of the format `F`: what a guest's `Guest` impl calls.
#[derive(Debug)]
pub struct Served<F>(PhantomData<F>);

impl<F: Format> Served<F> {
    /// Whether `F` reads the file at `path`; a path the model refuses is read by no format.
    pub fn handles(path: &str) -> bool {
        path.parse::<ContentPath>().is_ok_and(|path| F::handles(&path))
    }

    /// The file read; a path the model refuses, or a read the boundary cannot carry, is an
    /// empty read with the diagnostic saying why.
    pub fn read(path: &str, bytes: &[u8]) -> wire::Read {
        let read = match path.parse::<ContentPath>() {
            Ok(path) => F::read(&path, bytes),
            Err(refused) => {
                Self::refused(Problem::PluginValueMalformed { plugin: F::NAME.to_owned(), reason: refused.to_string() })
            }
        };
        wire::Read::try_from(&read).unwrap_or_else(|unmirrored| {
            let problem = Problem::PluginValueUnmirrored { reason: unmirrored.to_string() };
            let empty = wire::Read::try_from(&Self::refused(problem.clone()));
            empty.unwrap_or_else(|_| Self::bare(&Diagnostic::new(problem)))
        })
    }

    /// An empty read holding only `problem`.
    fn refused(problem: Problem) -> Read {
        let mut read = Read::new(Document::default(), Front::default(), None);
        read.diagnostics.push(Diagnostic::new(problem));
        read
    }

    /// An empty read in WIT holding only `diagnostic`, built without the mirror: what is sent
    /// when even an empty read does not cross.
    fn bare(diagnostic: &Diagnostic) -> wire::Read {
        wire::Read {
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
                aliases: None,
                block: None,
            },
            heading: None,
            inline_tags: Vec::new(),
            diagnostics: vec![Host::carried(diagnostic)],
            notices: Vec::new(),
        }
    }
}
