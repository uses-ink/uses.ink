//! The rules over the code itself, read from each crate's parsed module tree: function names of
//! one or two words, and no free functions outside a crate's entry points (docs/conventions.md,
//! Code). Test code and generated files are not checked.

use syn::visit::{self, Visit};

use super::workspace::{Attributes, Crate, Module, Role, Workspace};
use super::{Function, Problem, Problems, Rule};

/// No function or method name of three words or more: such a name is a missing type or module.
/// A trait impl's methods are named by the trait, so only the trait's own names count.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Words;

impl Words {
    /// The most words a name may have.
    const MOST: usize = 2;

    /// How many words a snake-case name has.
    fn count(name: &str) -> usize {
        name.trim_start_matches("r#").split('_').filter(|word| !word.is_empty()).count()
    }
}

impl Rule for Words {
    const NAME: &'static str = "words";

    fn check(&self, workspace: &Workspace, problems: &mut Problems) {
        for (krate, module, found) in Found::each(workspace) {
            if Self::count(&found.name) > Self::MOST {
                problems.push(Problem::Words(found.into_function(krate, module)));
            }
        }
    }
}

/// No free function, except a crate's entry points: the `pub` API and `main` of a target's root,
/// a function exported by its symbol name, and the few in [`Free::ALLOWED`].
#[derive(Debug, Clone, Copy)]
pub(crate) struct Free;

impl Free {
    /// Free functions allowed outside the entry points, by file and name, each with its reason.
    /// Kept tiny: a new entry needs a reason a type cannot answer.
    const ALLOWED: &'static [Exception] = &[];

    /// Whether a free function `found` in `module` is allowed.
    fn allows(module: &Module, found: &Found) -> bool {
        let entry = module.role == Role::Entry && (found.place == Place::Free(Reach::Public) || found.name == "main");
        let listed = Self::ALLOWED.iter().any(|e| module.path == std::path::Path::new(e.path) && e.name == found.name);
        entry || found.export || listed
    }
}

impl Rule for Free {
    const NAME: &'static str = "free";

    fn check(&self, workspace: &Workspace, problems: &mut Problems) {
        for (krate, module, found) in Found::each(workspace) {
            if matches!(found.place, Place::Free(_)) && !Self::allows(module, &found) {
                problems.push(Problem::Free(found.into_function(krate, module)));
            }
        }
    }
}

/// A free function [`Free`] allows, and why.
#[derive(Debug)]
struct Exception {
    path: &'static str,
    name: &'static str,
    /// Why no type can hold it; read by people, not by the rule.
    #[expect(dead_code, reason = "the reason is written for the reader of the list")]
    reason: &'static str,
}

/// A function as written, outside test code.
#[derive(Debug)]
struct Found {
    name: String,
    place: Place,
    export: bool,
}

/// Where a function is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Place {
    /// At module level: a free function.
    Free(Reach),
    /// Inside a block: a local function.
    Local,
    /// In an inherent impl.
    Method,
    /// Declared by a trait.
    Required,
}

/// Who can call a free function: a `pub` one at a target's root is an entry point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reach {
    /// `pub`.
    Public,
    /// Anything narrower.
    Private,
}

impl From<&syn::Visibility> for Reach {
    fn from(visibility: &syn::Visibility) -> Self {
        match visibility {
            syn::Visibility::Public(_) => Self::Public,
            _ => Self::Private,
        }
    }
}

impl Found {
    /// Every function of every checked module of every crate, with its crate and module.
    fn each(workspace: &Workspace) -> impl Iterator<Item = (&Crate, &Module, Self)> {
        workspace.crates.iter().flat_map(|krate| {
            krate.modules.iter().filter(|module| matches!(module.role, Role::Entry | Role::Inner)).flat_map(
                move |module| {
                    let mut walk = Walk::default();
                    walk.visit_file(&module.syntax);
                    walk.found.into_iter().map(move |found| (krate, module, found))
                },
            )
        })
    }

    fn into_function(self, krate: &Crate, module: &Module) -> Function {
        Function { krate: krate.name.clone(), path: module.path.clone(), name: self.name }
    }
}

/// A walk over one file collecting its functions, skipping test code and the methods a trait
/// names.
#[derive(Debug, Default)]
struct Walk {
    found: Vec<Found>,
    /// How many blocks deep the walk is: a function at depth 0 is free.
    depth: usize,
    /// Whether the walk is inside a trait impl, whose method names are the trait's.
    imposed: bool,
    /// How many inline `mod x { .. }` the walk is inside. A `pub` function in one is part of
    /// the module's API, not the target root's, so it is not an entry.
    inline: usize,
}

impl Walk {
    fn push(&mut self, sig: &syn::Signature, place: Place, attrs: &[syn::Attribute]) {
        let export = attrs.is_export() || sig.abi.is_some();
        self.found.push(Found { name: sig.ident.to_string(), place, export });
    }
}

impl<'ast> Visit<'ast> for Walk {
    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
        if !item.attrs.is_test() {
            let inline = usize::from(item.content.is_some());
            self.inline += inline;
            visit::visit_item_mod(self, item);
            self.inline -= inline;
        }
    }

    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        if item.attrs.is_test() {
            return;
        }
        let reach = if self.inline == 0 { Reach::from(&item.vis) } else { Reach::Private };
        let place = if self.depth == 0 { Place::Free(reach) } else { Place::Local };
        self.push(&item.sig, place, &item.attrs);
        visit::visit_item_fn(self, item);
    }

    fn visit_item_impl(&mut self, item: &'ast syn::ItemImpl) {
        if item.attrs.is_test() {
            return;
        }
        let outer = std::mem::replace(&mut self.imposed, item.trait_.is_some());
        visit::visit_item_impl(self, item);
        self.imposed = outer;
    }

    fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
        if item.attrs.is_test() {
            return;
        }
        if !self.imposed {
            self.push(&item.sig, Place::Method, &item.attrs);
        }
        visit::visit_impl_item_fn(self, item);
    }

    fn visit_item_trait(&mut self, item: &'ast syn::ItemTrait) {
        if !item.attrs.is_test() {
            visit::visit_item_trait(self, item);
        }
    }

    fn visit_trait_item_fn(&mut self, item: &'ast syn::TraitItemFn) {
        self.push(&item.sig, Place::Required, &item.attrs);
        visit::visit_trait_item_fn(self, item);
    }

    fn visit_item_foreign_mod(&mut self, _: &'ast syn::ItemForeignMod) {
        // foreign functions are named by the library that defines them
    }

    fn visit_block(&mut self, block: &'ast syn::Block) {
        self.depth += 1;
        visit::visit_block(self, block);
        self.depth -= 1;
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::layout::Gradual;

    fn walk(code: &str) -> Vec<(String, Place)> {
        let mut walk = Walk::default();
        walk.visit_file(&syn::parse_file(code).unwrap());
        walk.found.into_iter().map(|f| (f.name, f.place)).collect()
    }

    #[test]
    fn names_are_counted_in_words() {
        assert_eq!(Words::count("push"), 1);
        assert_eq!(Words::count("put_rust"), 2);
        assert_eq!(Words::count("push_text_node"), 3);
        assert_eq!(Words::count("_infallible"), 1);
        assert_eq!(Words::count("r#type_of_it"), 3);
    }

    #[test]
    fn functions_are_found_where_they_are_written() {
        let code = "fn free() { fn local() {} } impl X { fn method(&self) {} } trait T { fn required(&self); }";
        let found = walk(code);
        let expected = [
            ("free", Place::Free(Reach::Private)),
            ("local", Place::Local),
            ("method", Place::Method),
            ("required", Place::Required),
        ];
        assert_eq!(found, expected.map(|(n, p)| (n.to_owned(), p)));
    }

    #[test]
    fn test_code_and_trait_impls_are_skipped() {
        let code = "#[cfg(test)] mod tests { fn helper_for_tests() {} } #[test] fn a_long_test_name() {} \
                    impl Display for X { fn fmt_the_thing(&self) {} } extern \"C\" { fn foreign_named_thing(); }";
        assert_eq!(walk(code), []);
    }

    fn module(role: Role, code: &str) -> Module {
        Module { path: PathBuf::from("src/x.rs"), syntax: syn::parse_file(code).unwrap(), role }
    }

    fn free(name: &str, reach: Reach, export: bool) -> Found {
        Found { name: name.to_owned(), place: Place::Free(reach), export }
    }

    #[test]
    fn a_public_function_of_an_inline_module_is_not_a_root_entry() {
        let mut walk = Walk::default();
        walk.visit_file(&syn::parse_file("pub mod inner { pub fn read() {} }").unwrap());
        assert_eq!(walk.found.first().map(|f| f.place), Some(Place::Free(Reach::Private)));
    }

    #[test]
    fn only_a_roots_public_functions_and_its_main_are_entries() {
        let root = module(Role::Entry, "");
        let inner = module(Role::Inner, "");
        assert!(Free::allows(&root, &free("read", Reach::Public, false)));
        assert!(Free::allows(&root, &free("main", Reach::Private, false)));
        assert!(!Free::allows(&root, &free("helper", Reach::Private, false)));
        assert!(!Free::allows(&inner, &free("read", Reach::Public, false)));
        assert!(Free::allows(&inner, &free("symbol", Reach::Private, true)));
    }

    #[test]
    fn a_long_name_outside_the_enforced_crates_is_counted_not_failed() {
        let crate_with = |name: &str| Crate {
            name: name.to_owned(),
            dir: PathBuf::from(name),
            licence: None,
            dependencies: vec![],
            files: vec![],
            modules: vec![module(Role::Entry, "fn push_text_node() {}")],
        };
        let workspace = Workspace {
            root: PathBuf::from("/w"),
            crates: vec![crate_with("freehand"), crate_with("waived-crate")],
            aliased: vec![],
            entries: vec![],
        };
        let mut problems = Problems::default();
        Gradual(Words).check(&workspace, &mut problems);
        assert!(matches!(problems.found.as_slice(), [Problem::Words(function)] if function.krate == "freehand"));
        assert_eq!(problems.waived.get(&("waived-crate".to_owned(), Words::NAME)), Some(&1));
    }
}
