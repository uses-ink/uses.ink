//! `crates/core/token/token.toml` into `crates/core/token/src/generated.rs` (the pen's palette and the face stacks) and
//! `crates/core/token/token.css` (the same, as custom properties on `:root`). A `[look.<name>]` table
//! is a palette for light and dark, two faces and five sizes a theme wears: it is generated into
//! `generated.rs` as a `Look` that holds its colours and the stylesheet that declares them.

use std::fmt;

use heck::ToUpperCamelCase;
use proc_macro2::{Ident, TokenStream};
use quote::{ToTokens, format_ident, quote};
use token::{Rgb, Role, Scheme};

use crate::Task;
use crate::error::{Error, Flaw};
use crate::generate::output::{Files, Generator, Origin};

/// `cargo xtask token`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Token;

impl Token {
    const RUST: &'static str = "crates/core/token/src/generated.rs";
    const SHEET: &'static str = "crates/core/token/token.css";
}

impl Generator for Token {
    const TASK: Task = Task::Token;
    const SOURCE: &'static str = "crates/core/token/token.toml";
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
    looks: Vec<Look>,
}

impl TryFrom<toml::Table> for Tokens {
    type Error = Refusal;

    fn try_from(table: toml::Table) -> Result<Self, Refusal> {
        let mut tokens = Self { colours: Vec::new(), faces: Vec::new(), looks: Vec::new() };
        let mut looks = Vec::new();
        for (key, value) in table {
            match (key.as_str(), value) {
                ("look", toml::Value::Table(entries)) => looks = entries.into_iter().collect(),
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
        tokens.looks = looks.into_iter().map(|entry| Look::read(entry, &tokens.faces)).collect::<Result<_, _>>()?;
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
    /// `crates/core/token/src/generated.rs`: the `Colour` and `Face` enums.
    fn to_tokens(&self, out: &mut TokenStream) {
        let colours = Palette(&self.colours);
        let faces = Faces(&self.faces);
        let looks = Looks { looks: &self.looks, colours: &self.colours };
        out.extend(quote! {
            use crate::{Kind, Rgb, Role, Scheme, Unknown};
            #colours
            #faces
            #looks
        });
    }
}

/// An entry of `crates/core/token/token.toml` that is not a token: its name and what is wrong.
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

/// What is wrong with an entry of `crates/core/token/token.toml`.
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
    /// A look that is not a table of `light`, `dark`, `face` and `size`.
    Look,
    /// A scheme that does not hold each colour role once.
    Role,
    /// A size that is not a number of `rem`.
    Size,
    /// A look's face that `[face]` does not have.
    Missing,
}

impl fmt::Display for Fault {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::Section => "is not a section; the sections are `colour`, `face` and `look`",
            Self::Taken => {
                "is taken: a colour and a face, or two names that differ only by `-`, share a CSS name or a Rust variant"
            }
            Self::Name => "is not a name: write lowercase words joined by `-`",
            Self::Colour => "is not a colour: write `#rrggbb`",
            Self::Stack => {
                "is not a font stack: write a list of family names, without quotes, backslashes or control characters"
            }
            Self::Look => "is not a look: write a table with `light`, `face` and `size`, and `dark` if it has one",
            Self::Role => {
                "is not the colours of a scheme: write each of `ink`, `paper`, `muted`, `line`, `tint` and `accent` once, as `#rrggbb`"
            }
            Self::Size => "is not a size: write a number of `rem`, such as `1.125rem`",
            Self::Missing => "is not a face of `[face]`; a look has a `body`, a `hand` and a `mono` face, named there",
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

/// `crates/core/token/token.css`: every colour and face as a custom property on `:root`.
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

/// One look: the colours of a theme in light and dark, its two faces and its sizes.
#[derive(Debug)]
struct Look {
    name: Name,
    light: Shades,
    /// `None` for a look that has no dark scheme; it keeps its paper in a dark setting.
    dark: Option<Shades>,
    body: Vec<String>,
    hand: Vec<String>,
    mono: Vec<String>,
    sizes: Vec<String>,
}

/// The colour of each [`Role`] in one scheme, in the order of [`Role::ALL`].
#[derive(Debug)]
struct Shades(Vec<Rgb>);

impl Look {
    /// The colours of the look in `scheme`.
    const fn shades(&self, scheme: Scheme) -> &Shades {
        match (scheme, &self.dark) {
            (Scheme::Dark, Some(dark)) => dark,
            _ => &self.light,
        }
    }

    /// The sizes every look gives, as custom properties: `--size-small` and so on.
    const SIZES: [&'static str; 5] = ["small", "body", "h3", "h2", "h1"];

    /// The look `entry` is, its faces taken from `faces`.
    fn read((name, value): (String, toml::Value), faces: &[Face]) -> Result<Self, Refusal> {
        let name = Name::try_from(name)?;
        let toml::Value::Table(mut table) = value else { return Err(name.refuse(Fault::Look)) };
        let mut part = |key: &str| match table.remove(key) {
            Some(toml::Value::Table(part)) => Ok(part),
            _ => Err(Refusal { name: format!("{name}.{key}"), flaw: Fault::Look }),
        };
        let (light, face, size) = (part("light")?, part("face")?, part("size")?);
        let dark = match table.remove("dark") {
            None => None,
            Some(toml::Value::Table(dark)) => Some(dark),
            Some(_) => return Err(Refusal { name: format!("{name}.dark"), flaw: Fault::Look }),
        };
        if let Some(extra) = table.keys().next() {
            return Err(Refusal { name: format!("{name}.{extra}"), flaw: Fault::Look });
        }
        let stack = |face: &toml::Table, key: &str| {
            let at = || Refusal { name: format!("{name}.face.{key}"), flaw: Fault::Missing };
            let wanted = face.get(key).and_then(toml::Value::as_str).ok_or_else(at)?;
            faces.iter().find(|face| face.name.0 == wanted).map(|face| face.stack.clone()).ok_or_else(at)
        };
        let (body, hand, mono) = (stack(&face, "body")?, stack(&face, "hand")?, stack(&face, "mono")?);
        if let Some(extra) = face.keys().find(|key| !["body", "hand", "mono"].contains(&key.as_str())) {
            return Err(Refusal { name: format!("{name}.face.{extra}"), flaw: Fault::Missing });
        }
        Ok(Self {
            light: Shades::read(&light, &format!("{name}.light"))?,
            dark: dark.map(|dark| Shades::read(&dark, &format!("{name}.dark"))).transpose()?,
            sizes: Self::sizes(&size, &name)?,
            body,
            hand,
            mono,
            name,
        })
    }

    /// Each of [`Look::SIZES`] as written, a number of `rem`.
    fn sizes(table: &toml::Table, name: &Name) -> Result<Vec<String>, Refusal> {
        let at = |key: &str| Refusal { name: format!("{name}.size.{key}"), flaw: Fault::Size };
        if let Some(extra) = table.keys().find(|key| !Self::SIZES.contains(&key.as_str())) {
            return Err(at(extra));
        }
        Self::SIZES
            .iter()
            .map(|key| {
                let written = table.get(*key).and_then(toml::Value::as_str).ok_or_else(|| at(key))?;
                let number = written.strip_suffix("rem").ok_or_else(|| at(key))?;
                let digits = |part: &str| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit());
                let whole = number.split_once('.').map_or_else(|| digits(number), |(i, f)| digits(i) && digits(f));
                if whole { Ok(written.to_owned()) } else { Err(at(key)) }
            })
            .collect()
    }
}

impl Shades {
    /// One colour for each role of `table`, none more; `at` names the table in a refusal.
    fn read(table: &toml::Table, at: &str) -> Result<Self, Refusal> {
        let refuse = |role: &str| Refusal { name: format!("{at}.{role}"), flaw: Fault::Role };
        if let Some(extra) = table.keys().find(|key| !Role::ALL.iter().any(|role| role.name() == key.as_str())) {
            return Err(refuse(extra));
        }
        let colours = Role::ALL.iter().map(|role| {
            let value = table.get(role.name()).cloned().ok_or_else(|| refuse(role.name()))?;
            Colour::try_from((role.name().to_owned(), value)).map(|colour| colour.rgb).map_err(|_| refuse(role.name()))
        });
        colours.collect::<Result<_, _>>().map(Self)
    }
}

/// The `Look` enum of every look.
struct Looks<'a> {
    looks: &'a [Look],
    /// The paint box, whose colours that are no role join every look's stylesheet.
    colours: &'a [Colour],
}

impl Looks<'_> {
    /// The match arms giving each look's colour of a role in a scheme: one per scheme and role
    /// for a look with a dark scheme, one per role for a look without, which keeps its paper.
    fn arms(&self) -> TokenStream {
        let arms = self.looks.iter().map(|look| {
            let variant = look.name.variant();
            let schemes: &[Scheme] =
                if look.dark.is_some() { &[Scheme::Light, Scheme::Dark] } else { &[Scheme::Light] };
            let entries = schemes.iter().flat_map(|scheme| {
                let pattern = match (scheme, look.dark.is_some()) {
                    (_, false) => quote! { _ },
                    (Scheme::Light, true) => quote! { Scheme::Light },
                    (Scheme::Dark, true) => quote! { Scheme::Dark },
                };
                Role::ALL.iter().zip(&look.shades(*scheme).0).map(move |(role, Rgb(r, g, b))| {
                    let role = format_ident!("{}", role.name().to_upper_camel_case());
                    let pattern = pattern.clone();
                    quote! { (#pattern, Role::#role) => Rgb(#r, #g, #b) }
                })
            });
            quote! { Self::#variant => match (scheme, role) { #(#entries,)* } }
        });
        quote! { #(#arms,)* }
    }
}

impl ToTokens for Looks<'_> {
    fn to_tokens(&self, out: &mut TokenStream) {
        let names: Vec<&str> = self.looks.iter().map(|l| l.name.0.as_str()).collect();
        let variants: Vec<Ident> = self.looks.iter().map(|l| l.name.variant()).collect();
        let docs = self.looks.iter().map(|l| format!(" `{}`", l.name));
        let sheets = self.looks.iter().map(|l| LookSheet { look: l, colours: self.colours }.to_string());
        let arms = self.arms();
        out.extend(quote! {
            /// A look a theme can wear: its colours in light and dark, its faces and its sizes.
            #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
            pub enum Look { #( #[doc = #docs] #variants, )* }

            impl Look {
                /// Every look, in the order `token.toml` gives them.
                pub const ALL: &'static [Self] = &[#(Self::#variants),*];
                /// Every look's name, in the same order.
                pub const NAMES: &'static [&'static str] = &[#(#names),*];

                /// Its name.
                pub const fn name(self) -> &'static str {
                    match self { #(Self::#variants => #names,)* }
                }

                /// The colour of `role` in `scheme`.
                #[allow(clippy::match_same_arms, reason = "two roles may be the same colour in one look")]
                pub const fn colour(self, scheme: Scheme, role: Role) -> Rgb {
                    match self { #arms }
                }

                /// The stylesheet that declares the look's custom properties on `:root`, the dark
                /// ones under `prefers-color-scheme`.
                pub const fn css(self) -> &'static str {
                    match self { #(Self::#variants => #sheets,)* }
                }

                /// The look named `name`.
                pub fn named(name: &str) -> Result<Self, Unknown<'_>> {
                    match name {
                        #(#names => Ok(Self::#variants),)*
                        _ => Err(Unknown { kind: Kind::Look, name, known: Self::NAMES }),
                    }
                }
            }
        });
    }
}

/// A look as a stylesheet: light colours, the paint box's other colours, faces and sizes on
/// `:root`, then the dark colours when the look has them.
struct LookSheet<'a> {
    look: &'a Look,
    colours: &'a [Colour],
}

impl fmt::Display for LookSheet<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let look = self.look;
        let shades = |out: &mut fmt::Formatter<'_>, shades: &Shades, indent: &str| {
            Role::ALL
                .iter()
                .zip(&shades.0)
                .try_for_each(|(role, rgb)| writeln!(out, "{indent}--{}: {rgb};", role.name()))
        };
        writeln!(out, ":root {{")?;
        shades(out, &look.light, "  ")?;
        for colour in self.colours.iter().filter(|c| !Role::ALL.iter().any(|role| role.name() == c.name.0)) {
            writeln!(out, "{}", Declaration { name: &colour.name, value: colour.rgb })?;
        }
        writeln!(out, "  --face-body: {};", Stack(&look.body))?;
        writeln!(out, "  --face-hand: {};", Stack(&look.hand))?;
        writeln!(out, "  --face-mono: {};", Stack(&look.mono))?;
        for (key, size) in Look::SIZES.iter().zip(&look.sizes) {
            writeln!(out, "  --size-{key}: {size};")?;
        }
        writeln!(out, "}}")?;
        let Some(dark) = &look.dark else { return Ok(()) };
        writeln!(out, "\n@media (prefers-color-scheme: dark) {{\n  :root {{")?;
        shades(out, dark, "    ")?;
        writeln!(out, "  }}\n}}")
    }
}
