//! `token/token.toml` into `token/src/generated.rs`.

use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Stdio};

use heck::ToUpperCamelCase;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::error::Error;
use crate::output::Output;

const SOURCE: &str = "token/token.toml";
const TARGET: &str = "token/src/generated.rs";

/// One `#rrggbb` colour.
struct Colour {
    name: String,
    rgb: [u8; 3],
}

/// One font stack.
struct Face {
    name: String,
    stack: Vec<String>,
}

pub(crate) fn write(output: &mut Output) -> Result<(), Error> {
    let text = output.read(SOURCE)?;
    let table: toml::Table = toml::from_str(&text).map_err(|source| Error::Toml { path: SOURCE.into(), source })?;
    let mut colours = Vec::new();
    let mut faces = Vec::new();
    for (key, value) in table {
        match (key.as_str(), value) {
            ("colour", toml::Value::Table(entries)) => {
                for (name, value) in entries {
                    colours.push(colour(name, &value)?);
                }
            }
            ("face", toml::Value::Table(entries)) => {
                for (name, value) in entries {
                    faces.push(face(name, value)?);
                }
            }
            _ => return Err(Error::Token { name: key, why: "is not a section; the sections are `colour` and `face`" }),
        }
    }
    let code = rust(&colours, &faces);
    let text = format(output.root(), code)?;
    output.put(TARGET, text.as_bytes())
}

fn colour(name: String, value: &toml::Value) -> Result<Colour, Error> {
    let why = "is not a colour: write `#rrggbb`";
    let hex = value.as_str().and_then(|s| s.strip_prefix('#')).filter(|h| h.len() == 6);
    let Some(hex) = hex else { return Err(Error::Token { name, why }) };
    let byte = |at: usize| hex.get(at..at + 2).and_then(|pair| u8::from_str_radix(pair, 16).ok());
    match (byte(0), byte(2), byte(4)) {
        (Some(r), Some(g), Some(b)) => Ok(Colour { name, rgb: [r, g, b] }),
        _ => Err(Error::Token { name, why }),
    }
}

fn face(name: String, value: toml::Value) -> Result<Face, Error> {
    let why = "is not a font stack: write a list of family names";
    let toml::Value::Array(items) = value else { return Err(Error::Token { name, why }) };
    let stack: Option<Vec<String>> = items.into_iter().map(|v| v.as_str().map(str::to_owned)).collect();
    match stack {
        Some(stack) if !stack.is_empty() => Ok(Face { name, stack }),
        _ => Err(Error::Token { name, why }),
    }
}

fn rust(colours: &[Colour], faces: &[Face]) -> TokenStream {
    let colours = colour_enum(colours);
    let faces = face_enum(faces);
    quote! {
        use crate::{Rgb, Unknown};
        #colours
        #faces
    }
}

fn colour_enum(colours: &[Colour]) -> TokenStream {
    let names: Vec<&str> = colours.iter().map(|c| c.name.as_str()).collect();
    let variants: Vec<_> = names.iter().map(|n| format_ident!("{}", n.to_upper_camel_case())).collect();
    let docs = colours.iter().map(|c| format!(" `{}`, #{:02x}{:02x}{:02x}", c.name, c.rgb[0], c.rgb[1], c.rgb[2]));
    let rgbs = colours.iter().map(|c| {
        let [r, g, b] = c.rgb;
        quote! { Rgb(#r, #g, #b) }
    });
    quote! {
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
                    _ => Err(Unknown { kind: "colour", name, known: Self::NAMES }),
                }
            }
        }
    }
}

fn face_enum(faces: &[Face]) -> TokenStream {
    let face_names: Vec<&str> = faces.iter().map(|f| f.name.as_str()).collect();
    let face_variants: Vec<_> = face_names.iter().map(|n| format_ident!("{}", n.to_upper_camel_case())).collect();
    let face_docs = faces.iter().map(|f| format!(" `{}`: {}", f.name, f.stack.join(", ")));
    let stacks = faces.iter().map(|f| {
        let stack = &f.stack;
        quote! { &[#(#stack),*] }
    });
    quote! {
        /// A font stack.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum Face { #( #[doc = #face_docs] #face_variants, )* }

        impl Face {
            /// Every face, in the order `token.toml` gives them.
            pub const ALL: &'static [Self] = &[#(Self::#face_variants),*];
            /// Every face's name, in the same order.
            pub const NAMES: &'static [&'static str] = &[#(#face_names),*];

            /// Its name, which is also its CSS custom property without the `--`.
            pub const fn name(self) -> &'static str {
                match self { #(Self::#face_variants => #face_names,)* }
            }

            /// Its families, first choice first.
            pub const fn stack(self) -> &'static [&'static str] {
                match self { #(Self::#face_variants => #stacks,)* }
            }

            /// The face named `name`.
            pub fn named(name: &str) -> Result<Self, Unknown<'_>> {
                match name {
                    #(#face_names => Ok(Self::#face_variants),)*
                    _ => Err(Unknown { kind: "face", name, known: Self::NAMES }),
                }
            }
        }
    }
}

/// Printed by prettyplease, then rustfmt with the workspace's settings, so the file on disk is
/// what `cargo fmt` would leave.
fn format(root: &Path, code: TokenStream) -> Result<String, Error> {
    let file: syn::File = syn::parse2(code).map_err(|source| Error::Generated { path: TARGET.into(), source })?;
    let text = format!(
        "// Generated by `cargo xtask token` from `{SOURCE}`. Edit that file, not this one.\n\n{}",
        prettyplease::unparse(&file)
    );
    let fail = |why: String| Error::Rustfmt { path: TARGET.into(), why };
    let mut child = Command::new("rustfmt")
        .args(["--edition", "2024", "--emit", "stdout", "--config-path"])
        .arg(root.join("rustfmt.toml"))
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| fail(e.to_string()))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(text.as_bytes()).map_err(|e| fail(e.to_string()))?;
    }
    let done = child.wait_with_output().map_err(|e| fail(e.to_string()))?;
    if !done.status.success() {
        return Err(fail(String::from_utf8_lossy(&done.stderr).into_owned()));
    }
    String::from_utf8(done.stdout).map_err(|e| fail(e.to_string()))
}
