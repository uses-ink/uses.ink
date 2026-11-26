//! The Rust a checked catalog becomes: the enums in `model`, their wording in `diag`.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use super::catalog::{Catalog, Entry, Fault, Kind, Piece, Severity, Template};

/// `model/src/generated.rs`: `Problem` and `Announcement`, with codes, severities and faults.
pub(crate) fn model(catalog: &Catalog) -> TokenStream {
    let all: Vec<&Entry> = catalog.diagnostics.iter().chain(&catalog.notices).collect();
    let mut used: Vec<&str> = vec!["Fault", "Severity"];
    for kind in all.iter().flat_map(|e| e.fields.iter().map(|f| f.kind)) {
        let name = match kind {
            Kind::Path | Kind::Paths => "ContentPath",
            Kind::Route => "Route",
            Kind::Bytes => "Bytes",
            Kind::Span => "Span",
            Kind::Decision => "DecisionCode",
            Kind::Text | Kind::Count => continue,
        };
        if !used.contains(&name) {
            used.push(name);
        }
    }
    used.sort_unstable();
    let imports = used.iter().map(|name| format_ident!("{name}"));
    let problem = problem(&catalog.diagnostics);
    let announcement = announcement(&catalog.notices);
    quote! {
        use serde::Serialize;
        use crate::{#(#imports),*};
        #problem
        #announcement
    }
}

fn problem(entries: &[Entry]) -> TokenStream {
    let variants = variants(entries);
    let names = entries.iter().map(|e| format_ident!("{}", e.variant));
    let codes = entries.iter().map(|e| e.code.as_str());
    let severities = entries.iter().map(|e| match e.severity {
        Some(Severity::Error) | None => quote! { Severity::Error },
        Some(Severity::Warning) => quote! { Severity::Warning },
        Some(Severity::Note) => quote! { Severity::Note },
    });
    let faults = entries.iter().map(|e| match e.fault {
        Some(Fault::Author) => quote! { Fault::Author },
        Some(Fault::Source) => quote! { Fault::Source },
        Some(Fault::Operator) => quote! { Fault::Operator },
        Some(Fault::Ours) | None => quote! { Fault::Ours },
    });
    let patterns: Vec<_> = names.zip(entries).map(|(name, e)| pattern(&quote! { Self }, &name, e, &[])).collect();
    let codes: Vec<_> = codes.collect();
    quote! {
        /// A problem, one variant per diagnostic code in the catalog, with its typed fields.
        ///
        /// Not `#[non_exhaustive]`: `uses-ink-diag` matches it exhaustively, so a code without
        /// wording does not compile.
        #[derive(Clone, Debug, PartialEq, Eq, Serialize)]
        pub enum Problem { #(#variants)* }

        impl Problem {
            /// Every code, in catalog order.
            pub const CODES: &'static [&'static str] = &[#(#codes),*];

            /// Its dotted code, `noun.noun.reason`.
            pub const fn code(&self) -> &'static str {
                match self { #(#patterns => #codes,)* }
            }

            pub const fn severity(&self) -> Severity {
                match self { #(#patterns => #severities,)* }
            }

            /// Whose move it is to fix it.
            pub const fn fault(&self) -> Fault {
                match self { #(#patterns => #faults,)* }
            }
        }
    }
}

fn announcement(entries: &[Entry]) -> TokenStream {
    let variants = variants(entries);
    let patterns = entries.iter().map(|e| pattern(&quote! { Self }, &format_ident!("{}", e.variant), e, &[]));
    let codes: Vec<_> = entries.iter().map(|e| e.code.as_str()).collect();
    quote! {
        /// What a notice says, one variant per notice code in the catalog.
        #[derive(Clone, Debug, PartialEq, Eq, Serialize)]
        pub enum Announcement { #(#variants)* }

        impl Announcement {
            /// Every code, in catalog order.
            pub const CODES: &'static [&'static str] = &[#(#codes),*];

            pub const fn code(&self) -> &'static str {
                match self { #(#patterns => #codes,)* }
            }
        }
    }
}

/// `Kind::Variant`, `Kind::Variant { a, b, .. }` or `Kind::Variant { .. }`, binding `bound`.
fn pattern(kind: &TokenStream, name: &proc_macro2::Ident, entry: &Entry, bound: &[&str]) -> TokenStream {
    let bound = bound.iter().map(|f| format_ident!("{f}"));
    if entry.fields.is_empty() {
        quote! { #kind::#name }
    } else {
        quote! { #kind::#name { #(#bound,)* .. } }
    }
}

fn variants(entries: &[Entry]) -> Vec<TokenStream> {
    entries
        .iter()
        .map(|entry| {
            let name = format_ident!("{}", entry.variant);
            let code = &entry.code;
            let doc = format!(" `{code}`: {}", shown(&entry.message));
            let fields = entry.fields.iter().map(|field| {
                let name = format_ident!("{}", field.name);
                let kind = match field.kind {
                    Kind::Path => quote! { ContentPath },
                    Kind::Paths => quote! { Vec<ContentPath> },
                    Kind::Route => quote! { Route },
                    Kind::Text => quote! { String },
                    Kind::Count => quote! { u64 },
                    Kind::Bytes => quote! { Bytes },
                    Kind::Span => quote! { Span },
                    Kind::Decision => quote! { DecisionCode },
                };
                quote! { #name: #kind }
            });
            let body = if entry.fields.is_empty() {
                quote! {}
            } else {
                quote! { { #(#fields),* } }
            };
            quote! {
                #[doc = #doc]
                #[serde(rename = #code)]
                #name #body,
            }
        })
        .collect()
}

/// A template as it reads in the catalog, for docs.
fn shown(template: &Template) -> String {
    template
        .0
        .iter()
        .map(|piece| match piece {
            Piece::Text(text) => text.clone(),
            Piece::Field(name) => format!("{{{name}}}"),
        })
        .collect()
}

/// `diag/src/generated.rs`: the message and fix of every code.
pub(crate) fn wording(catalog: &Catalog) -> TokenStream {
    let problem_messages = arms(&catalog.diagnostics, &quote! { Problem }, |e| Some(&e.message));
    let problem_fixes = arms(&catalog.diagnostics, &quote! { Problem }, |e| e.fix.as_ref());
    let problem_has = has(&catalog.diagnostics, &quote! { Problem });
    let notice_messages = arms(&catalog.notices, &quote! { Announcement }, |e| Some(&e.message));
    let notice_fixes = arms(&catalog.notices, &quote! { Announcement }, |e| e.fix.as_ref());
    let notice_has = has(&catalog.notices, &quote! { Announcement });
    quote! {
        use std::fmt;
        use model::{Announcement, Problem};
        use crate::{Fix, Message, Paths};

        impl Message<'_> {
            pub(crate) fn problem(problem: &Problem, out: &mut fmt::Formatter<'_>) -> fmt::Result {
                match problem { #(#problem_messages)* }
            }

            pub(crate) fn announcement(announcement: &Announcement, out: &mut fmt::Formatter<'_>) -> fmt::Result {
                match announcement { #(#notice_messages)* }
            }
        }

        impl Fix<'_> {
            pub(crate) const fn problem_has(problem: &Problem) -> bool {
                match problem { #(#problem_has)* }
            }

            pub(crate) const fn announcement_has(announcement: &Announcement) -> bool {
                match announcement { #(#notice_has)* }
            }

            /// Writes nothing for a code without a fix; `Fix` is only made for one that has.
            pub(crate) fn problem(problem: &Problem, out: &mut fmt::Formatter<'_>) -> fmt::Result {
                match problem { #(#problem_fixes)* }
            }

            /// Writes nothing for a code without a fix; `Fix` is only made for one that has.
            pub(crate) fn announcement(announcement: &Announcement, out: &mut fmt::Formatter<'_>) -> fmt::Result {
                match announcement { #(#notice_fixes)* }
            }
        }
    }
}

fn has(entries: &[Entry], kind: &TokenStream) -> Vec<TokenStream> {
    entries
        .iter()
        .map(|entry| {
            let name = format_ident!("{}", entry.variant);
            let has = entry.fix.is_some();
            let pattern = pattern(kind, &name, entry, &[]);
            quote! { #pattern => #has, }
        })
        .collect()
}

/// One match arm per entry writing the template `pick` chooses, or nothing.
fn arms(entries: &[Entry], kind: &TokenStream, pick: impl Fn(&Entry) -> Option<&Template>) -> Vec<TokenStream> {
    entries
        .iter()
        .map(|entry| {
            let name = format_ident!("{}", entry.variant);
            let Some(template) = pick(entry) else {
                let pattern = pattern(kind, &name, entry, &[]);
                return quote! { #pattern => Ok(()), };
            };
            let used = template.fields();
            let pattern = pattern(kind, &name, entry, &used);
            // A list of paths is shown through `Paths`; every other type has its own Display.
            let wrapped = used.iter().filter_map(|f| {
                let field = entry.fields.iter().find(|d| d.name == *f)?;
                let ident = format_ident!("{f}");
                (field.kind == Kind::Paths).then(|| quote! { let #ident = Paths(#ident); })
            });
            let format: String = template
                .0
                .iter()
                .map(|piece| match piece {
                    Piece::Text(text) => text.replace('{', "{{").replace('}', "}}"),
                    Piece::Field(name) => format!("{{{name}}}"),
                })
                .collect();
            quote! {
                #pattern => {
                    #(#wrapped)*
                    write!(out, #format)
                }
            }
        })
        .collect()
}
