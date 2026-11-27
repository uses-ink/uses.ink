//! The workspace as the rules see it: its crates, their dependencies, every path in the tree, and
//! each crate's Rust, both as files and as the module tree its targets compile.

use std::fmt;
use std::path::PathBuf;

use serde::Deserialize;

use super::config::Config;

/// The workspace, read.
#[derive(Debug)]
pub(crate) struct Workspace {
    /// The directory holding the root manifest.
    pub(crate) root: PathBuf,
    /// The crates, in `cargo metadata`'s order.
    pub(crate) crates: Vec<Crate>,
    /// The crates `[workspace.dependencies]` names: by key, or by `package` when renamed.
    pub(crate) aliased: Vec<String>,
    /// Every path in the tree, relative to the root, outside [`Workspace::SKIP`].
    pub(crate) entries: Vec<PathBuf>,
    /// The rule data of `.config/layout.toml`.
    pub(crate) config: Config,
}

/// One crate of the workspace.
#[derive(Debug)]
pub(crate) struct Crate {
    pub(crate) name: String,
    /// Its directory, relative to the root.
    pub(crate) dir: PathBuf,
    pub(crate) licence: Option<String>,
    pub(crate) dependencies: Vec<Dependency>,
    /// Every `.rs` file in its directory and not in a nested crate's, compiled or not.
    pub(crate) files: Vec<Source>,
    /// Every module its targets compile, by path relative to the root.
    pub(crate) modules: Vec<Module>,
}

/// A dependency of a crate.
#[derive(Debug)]
pub(crate) struct Dependency {
    pub(crate) name: String,
    /// Whether it is a path dependency inside the workspace; a patched sibling outside is not
    /// ours.
    pub(crate) inside: bool,
    /// `dev` or `build`; none for a normal dependency.
    pub(crate) kind: Option<Kind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Kind {
    Dev,
    Build,
}

/// A Rust file, by path relative to the root, and its text.
#[derive(Debug)]
pub(crate) struct Source {
    pub(crate) path: PathBuf,
    pub(crate) text: String,
}

/// A module a target compiles: its file, parsed, and what part it plays.
pub(crate) struct Module {
    pub(crate) path: PathBuf,
    pub(crate) syntax: syn::File,
    pub(crate) role: Role,
}

impl fmt::Debug for Module {
    /// Its path and role; the syntax tree is left out.
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.debug_struct("Module").field("path", &self.path).field("role", &self.role).finish_non_exhaustive()
    }
}

/// What part a module plays in its crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Role {
    /// The root of a library, binary, example or build script.
    Entry,
    /// A module below a root.
    Inner,
    /// Test code: a test or bench target, or a module declared under `#[cfg(test)]`.
    Test,
    /// A file a generator writes.
    Generated,
}

impl Crate {
    /// Its directory with `/` between parts, as a crate name is built from it.
    pub(crate) fn place(&self) -> String {
        self.dir.components().map(|c| c.as_os_str().to_string_lossy()).collect::<Vec<_>>().join("/")
    }
}

/// What the rules read from a list of attributes.
pub(crate) trait Attributes {
    /// Whether they make the item test code: `#[test]`, or a `cfg` that needs `test`.
    fn is_test(&self) -> bool;
    /// Whether they export the item by its symbol name: `no_mangle` or `export_name`.
    fn is_export(&self) -> bool;
    /// The file a `#[path = ".."]` names.
    fn place(&self) -> Option<String>;
}

impl Attributes for [syn::Attribute] {
    fn is_test(&self) -> bool {
        self.iter().any(|attr| {
            let path = attr.path();
            let tested = path.segments.last().is_some_and(|s| s.ident == "test");
            let cfg = path.is_ident("cfg")
                && attr.meta.require_list().is_ok_and(|list| {
                    let words: Vec<String> = list.tokens.clone().into_iter().map(|t| t.to_string()).collect();
                    words.iter().any(|w| w == "test") && !words.iter().any(|w| w == "not")
                });
            tested || cfg
        })
    }

    fn is_export(&self) -> bool {
        let named = |word: &str| word == "no_mangle" || word == "export_name";
        self.iter().any(|attr| {
            let path = attr.path();
            path.get_ident().is_some_and(|i| named(&i.to_string()))
                || (path.is_ident("unsafe")
                    && attr
                        .meta
                        .require_list()
                        .is_ok_and(|list| list.tokens.clone().into_iter().any(|t| named(&t.to_string()))))
        })
    }

    fn place(&self) -> Option<String> {
        self.iter().find(|attr| attr.path().is_ident("path")).and_then(|attr| match &attr.meta {
            syn::Meta::NameValue(syn::MetaNameValue {
                value: syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(text), .. }),
                ..
            }) => Some(text.value()),
            _ => None,
        })
    }
}

#[cfg(test)]
impl Crate {
    /// A crate of ours at `dir` with `licence` and path dependencies on `dependencies`, each
    /// `(name, kind)`.
    pub(crate) fn sample(name: &str, dir: &str, licence: Option<&str>, dependencies: &[(&str, Option<Kind>)]) -> Self {
        Self {
            name: name.to_owned(),
            dir: PathBuf::from(dir),
            licence: licence.map(str::to_owned),
            dependencies: dependencies
                .iter()
                .map(|&(name, kind)| Dependency { name: name.to_owned(), inside: true, kind })
                .collect(),
            files: vec![],
            modules: vec![],
        }
    }
}

#[cfg(test)]
impl Workspace {
    /// A workspace of `crates`, with `aliased` as the aliases `[workspace.dependencies]` holds.
    pub(crate) fn sample(crates: Vec<Crate>, aliased: &[&str]) -> Self {
        Self {
            root: PathBuf::from("/w"),
            crates,
            aliased: aliased.iter().map(|name| (*name).to_owned()).collect(),
            entries: vec![],
            config: Config::default(),
        }
    }
}
