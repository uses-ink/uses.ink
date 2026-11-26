//! The Rust a checked catalog becomes: the enums in `model`, their wording in `diag`.

use std::fmt;

use proc_macro2::{Ident, TokenStream};
use quote::{ToTokens, format_ident, quote};

use super::catalog::{Catalog, Entry, Fault, Kind, Part, Piece, Severity, Template};

impl Catalog {
    /// `model/src/generated.rs`: `Problem` and `Announcement`, with codes, severities and faults.
    pub(crate) fn model(&self) -> TokenStream {
        let kinds = self.diagnostics.iter().flat_map(|e| &e.fields).chain(self.notices.iter().flat_map(|e| &e.fields));
        let mut used: Vec<&str> = vec!["Fault", "Severity"];
        for name in kinds.filter_map(|field| field.kind.import()) {
            if !used.contains(&name) {
                used.push(name);
            }
        }
        used.sort_unstable();
        let imports = used.iter().map(|name| format_ident!("{name}"));
        let problem = self.problem();
        let announcement = self.announcement();
        let coded =
            [Self::coded(&self.diagnostics, &quote! { Problem }), Self::coded(&self.notices, &quote! { Announcement })];
        quote! {
            use serde::Serialize;
            use crate::coded::{Coded, Field, Refused};
            use crate::{#(#imports),*};
            #problem
            #announcement
            #(#coded)*
        }
    }

    fn problem(&self) -> TokenStream {
        let entries = &self.diagnostics;
        let variants = entries.iter().map(Entry::declaration);
        let codes: Vec<&str> = entries.iter().map(|e| e.code.as_str()).collect();
        let severities = entries.iter().map(|e| e.class.severity);
        let faults = entries.iter().map(|e| e.class.fault);
        let patterns: Vec<TokenStream> = entries.iter().map(|e| e.pattern(&quote! { Self }, &[])).collect();
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

                /// How bad it is.
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

    /// `From<&T> for Coded` and `TryFrom<Coded> for T`, for `T` the enum of `entries`: each
    /// variant's fields in catalog order.
    fn coded<C>(entries: &[Entry<C>], owner: &TokenStream) -> TokenStream {
        let names: Vec<Vec<Ident>> =
            entries.iter().map(|e| e.fields.iter().map(|f| format_ident!("{}", f.name)).collect()).collect();
        let patterns = entries.iter().zip(&names).map(|(e, names)| {
            let bound: Vec<&str> = e.fields.iter().map(|f| f.name.as_str()).collect();
            let (pattern, code) = (e.pattern(owner, &bound), e.code.as_str());
            let values = e.fields.iter().zip(names).map(|(f, name)| f.kind.copied(name));
            quote! { #pattern => Coded::new(#code, vec![#(Field::from(#values)),*]) }
        });
        let codes = entries.iter().map(|e| e.code.as_str());
        let made = entries.iter().zip(&names).map(|(e, names)| {
            let name = e.ident();
            if names.is_empty() {
                quote! { Self::#name }
            } else {
                quote! { Self::#name { #(#names: fields.next()?),* } }
            }
        });
        quote! {
            impl From<&#owner> for Coded {
                fn from(erased: &#owner) -> Self {
                    match erased { #(#patterns,)* }
                }
            }

            impl TryFrom<Coded> for #owner {
                type Error = Refused;

                fn try_from(coded: Coded) -> Result<Self, Refused> {
                    let mut fields = coded.into_fields();
                    let made = match fields.code().to_owned().as_str() {
                        #(#codes => #made,)*
                        _ => return fields.unknown(),
                    };
                    fields.end(made)
                }
            }
        }
    }

    fn announcement(&self) -> TokenStream {
        let entries = &self.notices;
        let variants = entries.iter().map(Entry::declaration);
        let patterns = entries.iter().map(|e| e.pattern(&quote! { Self }, &[]));
        let codes: Vec<&str> = entries.iter().map(|e| e.code.as_str()).collect();
        quote! {
            /// What a notice says, one variant per notice code in the catalog.
            #[derive(Clone, Debug, PartialEq, Eq, Serialize)]
            pub enum Announcement { #(#variants)* }

            impl Announcement {
                /// Every code, in catalog order.
                pub const CODES: &'static [&'static str] = &[#(#codes),*];

                /// Its dotted code, `noun.noun.reason`.
                pub const fn code(&self) -> &'static str {
                    match self { #(#patterns => #codes,)* }
                }
            }
        }
    }

    /// `diag/src/generated.rs`: the message and fix of every code.
    pub(crate) fn wording(&self) -> TokenStream {
        let (problem, announcement) = (quote! { Problem }, quote! { Announcement });
        let problem_messages = self.diagnostics.iter().map(|e| e.arm(&problem, Part::Message));
        let problem_fixes = self.diagnostics.iter().map(|e| e.arm(&problem, Part::Fix));
        let problem_has = self.diagnostics.iter().map(|e| e.has(&problem));
        let notice_messages = self.notices.iter().map(|e| e.arm(&announcement, Part::Message));
        let notice_fixes = self.notices.iter().map(|e| e.arm(&announcement, Part::Fix));
        let notice_has = self.notices.iter().map(|e| e.has(&announcement));
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
}

impl<C> Entry<C> {
    fn ident(&self) -> Ident {
        format_ident!("{}", self.variant)
    }

    /// `Owner::Variant`, `Owner::Variant { a, b, .. }` or `Owner::Variant { .. }`, binding
    /// `bound`.
    fn pattern(&self, owner: &impl ToTokens, bound: &[&str]) -> TokenStream {
        let name = self.ident();
        let bound = bound.iter().map(|f| format_ident!("{f}"));
        if self.fields.is_empty() {
            quote! { #owner::#name }
        } else {
            quote! { #owner::#name { #(#bound,)* .. } }
        }
    }

    /// Its variant in the enum, with its doc, its serde name and its typed fields.
    fn declaration(&self) -> TokenStream {
        let name = self.ident();
        let code = &self.code;
        let doc = format!(" `{code}`: {}", self.message);
        let fields = self.fields.iter().map(|field| {
            let name = format_ident!("{}", field.name);
            let kind = field.kind.rust();
            let doc = format!(" {}", field.doc);
            quote! { #[doc = #doc] #name: #kind }
        });
        let body = if self.fields.is_empty() {
            quote! {}
        } else {
            quote! { { #(#fields),* } }
        };
        quote! {
            #[doc = #doc]
            #[serde(rename = #code)]
            #name #body,
        }
    }

    /// The match arm saying whether it has a fix.
    fn has(&self, owner: &impl ToTokens) -> TokenStream {
        let has = self.fix.is_some();
        let pattern = self.pattern(owner, &[]);
        quote! { #pattern => #has, }
    }

    /// The match arm writing its template of `part`, or nothing.
    fn arm(&self, owner: &impl ToTokens, part: Part) -> TokenStream {
        let Some(template) = self.template(part) else {
            let pattern = self.pattern(owner, &[]);
            return quote! { #pattern => Ok(()), };
        };
        let used = template.fields();
        let pattern = self.pattern(owner, &used);
        // A list of paths is shown through `Paths`; every other type has its own Display.
        let wrapped = used.iter().filter_map(|f| {
            let field = self.fields.iter().find(|d| d.name == *f)?;
            let ident = format_ident!("{f}");
            (field.kind == Kind::Paths).then(|| quote! { let #ident = Paths(#ident); })
        });
        let format = template.format();
        quote! {
            #pattern => {
                #(#wrapped)*
                write!(out, #format)
            }
        }
    }
}

impl Kind {
    /// The Rust type of a field of this kind.
    fn rust(self) -> TokenStream {
        match self {
            Self::Path => quote! { ContentPath },
            Self::Paths => quote! { Vec<ContentPath> },
            Self::Route => quote! { Route },
            Self::Text => quote! { String },
            Self::Count => quote! { u64 },
            Self::Bytes => quote! { Bytes },
            Self::Span => quote! { Span },
            Self::Decision => quote! { AnyCode },
        }
    }

    /// The value of the field bound as `name`, owned: copied or cloned, as its type allows.
    fn copied(self, name: &Ident) -> TokenStream {
        match self {
            Self::Count | Self::Bytes | Self::Span | Self::Decision => quote! { *#name },
            Self::Path | Self::Paths | Self::Route | Self::Text => quote! { #name.clone() },
        }
    }

    /// The model type the generated file imports for it, if any.
    const fn import(self) -> Option<&'static str> {
        match self {
            Self::Path | Self::Paths => Some("ContentPath"),
            Self::Route => Some("Route"),
            Self::Bytes => Some("Bytes"),
            Self::Span => Some("Span"),
            Self::Decision => Some("AnyCode"),
            Self::Text | Self::Count => None,
        }
    }
}

impl ToTokens for Severity {
    fn to_tokens(&self, out: &mut TokenStream) {
        out.extend(match self {
            Self::Error => quote! { Severity::Error },
            Self::Warning => quote! { Severity::Warning },
            Self::Note => quote! { Severity::Note },
        });
    }
}

impl ToTokens for Fault {
    fn to_tokens(&self, out: &mut TokenStream) {
        out.extend(match self {
            Self::Author => quote! { Fault::Author },
            Self::Source => quote! { Fault::Source },
            Self::Operator => quote! { Fault::Operator },
            Self::Ours => quote! { Fault::Ours },
        });
    }
}

impl Template {
    /// The format string `write!` takes for it: text with its braces doubled, fields as holes.
    fn format(&self) -> String {
        self.0
            .iter()
            .map(|piece| match piece {
                Piece::Text(text) => text.replace('{', "{{").replace('}', "}}"),
                Piece::Field(name) => format!("{{{name}}}"),
            })
            .collect()
    }
}

impl fmt::Display for Template {
    /// The template as it reads in the catalog, for docs.
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.iter().try_for_each(|piece| match piece {
            Piece::Text(text) => out.write_str(text),
            Piece::Field(name) => write!(out, "{{{name}}}"),
        })
    }
}
