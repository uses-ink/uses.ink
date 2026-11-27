//! `token/token.toml` into `token/src/generated.rs` (the pen's palette and the face stacks) and
//! `token/token.css` (the same, as custom properties on `:root`).

use std::fmt;

use heck::ToUpperCamelCase;
use proc_macro2::{Ident, TokenStream};
use quote::{ToTokens, format_ident, quote};
use token::Rgb;

use crate::Task;
use crate::error::{Error, Flaw};
use crate::generate::output::{Files, Generator, Origin};

/// `cargo xtask token`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Token;

impl Token {
    const RUST: &'static str = "token/src/generated.rs";
    const SHEET: &'static str = "token/token.css";
}

impl Generator for Token {
    const TASK: Task = Task::Token;
    const SOURCE: &'static str = "token/token.toml";
    type Source = Tokens;

    fn generate(&self, tokens: &Tokens, files: &mut Files<'_>) -> Result<(), Error> {
        files.rust(Self::RUST, tokens.to_token_stream())?;
        files.put(Self::SHEET, Sheet { tokens, origin: files.origin() }.to_string())
    }
}

/// The token file, read and checked: every colour and face, in file order.
#[derive(Debug)]
pub(crate) struct Tokens {
    colours: Vec<Colour>,
    faces: Vec<Face>,
}

impl TryFrom<toml::Table> for Tokens {
    type Error = Refusal;

    fn try_from(table: toml::Table) -> Result<Self, Refusal> {
        let mut tokens = Self { colours: Vec::new(), faces: Vec::new() };
        for (key, value) in table {
            match (key.as_str(), value) {
                ("colour", toml::Value::Table(entries)) => {
                    tokens.colours = entries.into_iter().map(Colour::try_from).collect::<Result<_, _>>()?;
                }
                ("face", toml::Value::Table(entries)) => {
                    tokens.faces = entries.into_iter().map(Face::try_from).collect::<Result<_, _>>()?;
                }
                _ => return Err(Refusal { name: key, flaw: Fault::Section }),
            }
        }
        tokens.unique()?;
        Ok(tokens)
    }
}

impl Tokens {
    /// Every name is one CSS custom property and one Rust variant, so no two may share either.
    fn unique(&self) -> Result<(), Refusal> {
        let names = self.colours.iter().map(|c| &c.name).chain(self.faces.iter().map(|f| &f.name));
        let mut seen: Vec<(String, &Name)> = Vec::new();
        for name in names {
            let variant = name.0.to_upper_camel_case();
            if seen.iter().any(|(v, n)| *n == name || *v == variant) {
                return Err(Refusal { name: name.0.clone(), flaw: Fault::Taken });
            }
            seen.push((variant, name));
        }
        Ok(())
    }
}

impl ToTokens for Tokens {
    /// `token/src/generated.rs`: the `Colour` and `Face` enums.
    fn to_tokens(&self, out: &mut TokenStream) {
        let colours = Palette(&self.colours);
        let faces = Faces(&self.faces);
        out.extend(quote! {
            use crate::{Kind, Rgb, Unknown};
            #colours
            #faces
        });
    }
}

/// An entry of `token/token.toml` that is not a token: its name and what is wrong.
#[derive(Debug)]
pub(crate) struct Refusal {
    name: String,
    flaw: Fault,
}

impl fmt::Display for Refusal {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "`{}` {}", self.name, self.flaw)
    }
}

impl From<Refusal> for Flaw {
    fn from(refusal: Refusal) -> Self {
        Self::Token(refusal)
    }
}

/// What is wrong with an entry of `token/token.toml`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Fault {
    /// A top-level key other than `colour` and `face`.
    Section,
    /// A name another entry already makes into the same CSS name or Rust variant.
    Taken,
    /// A name that is not lowercase words joined by `-`.
    Name,
    /// A colour that is not `#rrggbb`.
    Colour,
    /// A face that is not a list of plain family names.
    Stack,
}

impl fmt::Display for Fault {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::Section => "is not a section; the sections are `colour` and `face`",
            Self::Taken => {
                "is taken: a colour and a face, or two names that differ only by `-`, share a CSS name or a Rust variant"
            }
            Self::Name => "is not a name: write lowercase words joined by `-`",
            Self::Colour => "is not a colour: write `#rrggbb`",
            Self::Stack => {
                "is not a font stack: write a list of family names, without quotes, backslashes or control characters"
            }
        })
    }
}

/// A token's name, which is a Rust variant and a CSS custom property: lowercase words joined
/// by `-`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Name(String);

impl TryFrom<String> for Name {
    type Error = Refusal;

    fn try_from(name: String) -> Result<Self, Refusal> {
        let word = |w: &str| !w.is_empty() && w.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit());
        let starts = name.bytes().next().is_some_and(|b| b.is_ascii_lowercase());
        if starts && name.split('-').all(word) { Ok(Self(name)) } else { Err(Refusal { name, flaw: Fault::Name }) }
    }
}

impl Name {
    /// Its Rust variant: `ink-soft` is `InkSoft`.
    fn variant(&self) -> Ident {
        format_ident!("{}", self.0.to_upper_camel_case())
    }

    /// A refusal of the entry it names.
    fn refuse(&self, flaw: Fault) -> Refusal {
        Refusal { name: self.0.clone(), flaw }
    }
}

impl fmt::Display for Name {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(&self.0)
    }
}

/// One `#rrggbb` colour.
#[derive(Debug)]
struct Colour {
    name: Name,
    rgb: Rgb,
}

impl TryFrom<(String, toml::Value)> for Colour {
    type Error = Refusal;

    fn try_from((name, value): (String, toml::Value)) -> Result<Self, Refusal> {
        let name = Name::try_from(name)?;
        let hex = value
            .as_str()
            .and_then(|s| s.strip_prefix('#'))
            .filter(|h| h.len() == 6 && h.bytes().all(|b| b.is_ascii_hexdigit()));
        let Some(hex) = hex else { return Err(name.refuse(Fault::Colour)) };
        let byte = |at: usize| hex.get(at..at + 2).and_then(|pair| u8::from_str_radix(pair, 16).ok());
        match (byte(0), byte(2), byte(4)) {
            (Some(r), Some(g), Some(b)) => Ok(Self { name, rgb: Rgb(r, g, b) }),
            _ => Err(name.refuse(Fault::Colour)),
        }
    }
}

/// One font stack.
#[derive(Debug)]
struct Face {
    name: Name,
    stack: Vec<String>,
}

impl TryFrom<(String, toml::Value)> for Face {
    type Error = Refusal;

    fn try_from((name, value): (String, toml::Value)) -> Result<Self, Refusal> {
        let name = Name::try_from(name)?;
        let toml::Value::Array(items) = value else { return Err(name.refuse(Fault::Stack)) };
        // a family is written inside double quotes, so it may not hold one, a backslash or a
        // control character (CSS reads CR and FF as newlines)
        let plain =
            |family: &&str| !family.is_empty() && !family.chars().any(|c| c == '"' || c == '\\' || c.is_control());
        let stack: Option<Vec<String>> =
            items.into_iter().map(|v| v.as_str().filter(plain).map(str::to_owned)).collect();
        match stack {
            Some(stack) if !stack.is_empty() => Ok(Self { name, stack }),
            _ => Err(name.refuse(Fault::Stack)),
        }
    }
}

/// The `Colour` enum of every colour.
struct Palette<'a>(&'a [Colour]);

impl ToTokens for Palette<'_> {
    fn to_tokens(&self, out: &mut TokenStream) {
        let names: Vec<&str> = self.0.iter().map(|c| c.name.0.as_str()).collect();
        let variants: Vec<Ident> = self.0.iter().map(|c| c.name.variant()).collect();
        let docs = self.0.iter().map(|c| format!(" `{}`, {}", c.name, c.rgb));
        let rgbs = self.0.iter().map(|c| {
            let Rgb(r, g, b) = c.rgb;
            quote! { Rgb(#r, #g, #b) }
        });
        out.extend(quote! {
            /// A colour of the paint box.
            #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
            pub enum Colour { #( #[doc = #docs] #variants, )* }

            impl Colour {
                /// Every colour, in the order `token.toml` gives them.
                pub const ALL: &'static [Self] = &[#(Self::#variants),*];
                /// Every colour's name, in the same order.
                pub const NAMES: &'static [&'static str] = &[#(#names),*];

                /// Its name, which is also its CSS custom property without the `--`.
                pub const fn name(self) -> &'static str {
                    match self { #(Self::#variants => #names,)* }
                }

                /// Its value.
                pub const fn rgb(self) -> Rgb {
                    match self { #(Self::#variants => #rgbs,)* }
                }

                /// The colour named `name`.
                pub fn named(name: &str) -> Result<Self, Unknown<'_>> {
                    match name {
                        #(#names => Ok(Self::#variants),)*
                        _ => Err(Unknown { kind: Kind::Colour, name, known: Self::NAMES }),
                    }
                }
            }
        });
    }
}

/// The `Face` enum of every font stack.
struct Faces<'a>(&'a [Face]);

impl ToTokens for Faces<'_> {
    fn to_tokens(&self, out: &mut TokenStream) {
        let names: Vec<&str> = self.0.iter().map(|f| f.name.0.as_str()).collect();
        let variants: Vec<Ident> = self.0.iter().map(|f| f.name.variant()).collect();
        let docs = self.0.iter().map(|f| format!(" `{}`: {}", f.name, f.stack.join(", ")));
        let stacks = self.0.iter().map(|f| {
            let stack = &f.stack;
            quote! { &[#(#stack),*] }
        });
        out.extend(quote! {
            /// A font stack.
            #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
            pub enum Face { #( #[doc = #docs] #variants, )* }

            impl Face {
                /// Every face, in the order `token.toml` gives them.
                pub const ALL: &'static [Self] = &[#(Self::#variants),*];
                /// Every face's name, in the same order.
                pub const NAMES: &'static [&'static str] = &[#(#names),*];

                /// Its name, which is also its CSS custom property without the `--`.
                pub const fn name(self) -> &'static str {
                    match self { #(Self::#variants => #names,)* }
                }

                /// Its families, first choice first.
                pub const fn stack(self) -> &'static [&'static str] {
                    match self { #(Self::#variants => #stacks,)* }
                }

                /// The face named `name`.
                pub fn named(name: &str) -> Result<Self, Unknown<'_>> {
                    match name {
                        #(#names => Ok(Self::#variants),)*
                        _ => Err(Unknown { kind: Kind::Face, name, known: Self::NAMES }),
                    }
                }
            }
        });
    }
}

/// `token/token.css`: every colour and face as a custom property on `:root`.
struct Sheet<'a> {
    tokens: &'a Tokens,
    origin: Origin,
}

impl fmt::Display for Sheet<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(out, "/* {} */", self.origin)?;
        writeln!(out, "\n:root {{")?;
        for colour in &self.tokens.colours {
            writeln!(out, "{}", Declaration { name: &colour.name, value: colour.rgb })?;
        }
        for face in &self.tokens.faces {
            writeln!(out, "{}", Declaration { name: &face.name, value: Stack(&face.stack) })?;
        }
        writeln!(out, "}}")
    }
}

/// One custom property: `  --name: value;`.
struct Declaration<'a, V> {
    name: &'a Name,
    value: V,
}

impl<V: fmt::Display> fmt::Display for Declaration<'_, V> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "  --{}: {};", self.name, self.value)
    }
}

/// A font stack as CSS writes it: families joined by `, `.
struct Stack<'a>(&'a [String]);

impl fmt::Display for Stack<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0
            .iter()
            .enumerate()
            .try_for_each(|(i, family)| write!(out, "{}{}", if i == 0 { "" } else { ", " }, Family(family)))
    }
}

/// A font family as CSS writes it: a generic keyword bare, any other name in double quotes.
/// A name never holds a quote, a backslash or a control character (`Face` refuses them).
struct Family<'a>(&'a str);

impl Family<'_> {
    /// Font families CSS names by keyword, written without quotes.
    const GENERIC: &'static [&'static str] = &[
        "serif",
        "sans-serif",
        "monospace",
        "cursive",
        "fantasy",
        "system-ui",
        "ui-serif",
        "ui-sans-serif",
        "ui-monospace",
        "ui-rounded",
        "math",
        "emoji",
        "fangsong",
    ];
}

impl fmt::Display for Family<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        if Self::GENERIC.contains(&self.0) { out.write_str(self.0) } else { write!(out, "\"{}\"", self.0) }
    }
}
