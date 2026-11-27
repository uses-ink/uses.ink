---
number: "0039"
title: "Highlighting is a host capability, through arborium"
status: accepted
date: 2026-11-26
area: engine
supersedes: []
---

## 0039 Highlighting is a host capability, through arborium

Accepted, owner. Code is highlighted by the host, behind a WIT interface `ink:host/highlight`
next to `pen` and `text`: a theme asks for a code block's spans by language and gets classed
spans back, then styles the classes. The host implements it with arborium (tree-sitter grammars,
only the languages we enable as features). A theme never links a highlighter. **Why:** every
theme highlights the same way, as with the pen; tree-sitter parses more accurately than regex
grammars; C grammars stay in the native host, so themes stay small pure-Rust components; and
giallo's prebuilt bundle needed `zstd-sys` (C) inside the theme and loaded every grammar up front
(about 40 ms). **Fallback:** giallo without its `dump` feature, with grammars bundled by an xtask
generator, if arborium's C toolchain gets in the way. **Reversal:** a theme needs a highlighting
style tree-sitter's capture names cannot express.

As built (2026-11-26), natively until `ink:host/highlight` exists in WIT: the trait is
`model::highlight::Highlight`, in model and not the engine, because the engine never highlights
and the theme and host-native both reach model already. It returns `Run`s (an optional `Capture`
and the text they cover) or a typed `Unhighlighted` (`Unknown`, or `Failed` with the host's
error), so `code.language.unknown` comes from a variant, never a string. `Capture` is a closed
set of 27; host-native maps tree-sitter capture names onto it through `arborium-theme`'s slot
table, in one exhaustive `match`, and flattens nested and crossing captures so the innermost
wins. arborium 2.18 with 14 grammars (bash, c, css, diff, go, html, javascript, json, markdown,
python, rust, toml, typescript, yaml) builds its C in about 12 s on a first release build and
adds 4.7 MB to `ink` (13.3 MB to 18.0 MB). A grammar compiles on first use, once per process:
31 ms for Rust, 36 ms for TypeScript, 3 to 5 ms for Markdown or Python in release; 218 lines of
Rust then highlight in 2 ms. Its licences are MIT OR Apache-2.0, so `deny.toml` needs no
exception, and giallo, zstd and their exceptions are gone. host-native builds for
`wasm32-unknown-unknown` with the system clang (arborium ships its own sysroot), so the plan
holds if the host ever runs in wasm; the theme no longer links any highlighter, nor C.

As measured (2026-11-27, `research/performance.md`): highlighting is 79% of rendering a post
with eight code blocks, nearly all of it tree-sitter's parse and query; a 25-line Rust block
takes 2.7 million instructions, about 270 us. Reusing the parser between calls saves 0.5%, so
`Grammars` still makes one per call.
