//! Inputs for benchmarks, generated: the same seed writes the same bytes on every machine, so a
//! measurement taken today and one taken after a change read the same text.
//!
//! This is bench data, not test data: no golden is compared against it, and no test depends on
//! its exact words.
//!
//! - [`Post`] is a post of about 2,000 words using every construct the markdown format reads:
//!   headings, lists, task lists, tables, footnotes, code in Rust, Python and TypeScript, math,
//!   callouts, definition lists and the raw HTML the allowlist keeps.
//! - [`Flood`] is a body of many short paragraphs: 20,000 of them are about 1.6 MB, the
//!   pathological file of the M1 safety review.
//! - [`Repeated`] is one block written again and again: a heading every anchor of which is
//!   taken, or a `<details>` never closed.
//! - [`Notes`] is a body of many footnotes, each referenced once.
//! - [`Blog`] is a folder of posts linking to one another, with a few pages beside them.
//!
//! ```
//! let post = uses_ink_fixture::corpus::Post { seed: 1, links: 0 }.to_string();
//! assert!(post.split_whitespace().count() > 2_000);
//! ```

use std::fmt::{self, Write as _};

/// The words prose is drawn from: common English, long and short, so slugs, line breaking and
/// escaping see ordinary text.
const WORDS: &[&str] = &[
    "the",
    "of",
    "and",
    "to",
    "in",
    "a",
    "is",
    "that",
    "for",
    "it",
    "as",
    "was",
    "with",
    "be",
    "by",
    "on",
    "not",
    "he",
    "this",
    "are",
    "or",
    "his",
    "from",
    "at",
    "which",
    "but",
    "have",
    "an",
    "had",
    "they",
    "you",
    "were",
    "their",
    "one",
    "all",
    "we",
    "can",
    "her",
    "has",
    "there",
    "been",
    "if",
    "more",
    "when",
    "will",
    "would",
    "who",
    "so",
    "no",
    "garden",
    "repository",
    "markdown",
    "render",
    "theme",
    "pencil",
    "folder",
    "page",
    "site",
    "build",
    "deploy",
    "snapshot",
    "parser",
    "grammar",
    "highlight",
    "footnote",
    "heading",
    "paragraph",
    "table",
    "column",
    "measure",
    "quickly",
    "slowly",
    "carefully",
    "deterministic",
    "reader",
    "writer",
    "author",
    "letter",
    "number",
    "evening",
    "morning",
    "window",
    "kitchen",
    "library",
    "station",
    "harbour",
    "mountain",
    "river",
    "question",
    "answer",
    "between",
    "through",
    "without",
    "another",
    "nothing",
    "everything",
    "because",
    "although",
    "tomorrow",
];

/// A deterministic stream of draws: xorshift64*, seeded.
#[derive(Clone, Debug)]
struct Draws(u64);

impl Draws {
    /// The stream of `seed`; every seed, zero included, gives a stream of its own.
    const fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    /// The next draw.
    const fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// A draw below `bound`.
    fn below(&mut self, bound: usize) -> usize {
        usize::try_from(self.next() >> 33).unwrap_or(0) % bound.max(1)
    }

    /// A word of [`WORDS`].
    fn word(&mut self) -> &'static str {
        let at = self.below(WORDS.len());
        WORDS.get(at).copied().unwrap_or("word")
    }

    /// `count` words, separated by spaces.
    fn words(&mut self, count: usize, out: &mut impl fmt::Write) -> fmt::Result {
        for n in 0..count {
            if n > 0 {
                out.write_char(' ')?;
            }
            out.write_str(self.word())?;
        }
        Ok(())
    }

    /// A sentence of `count` words with a capital and a full stop, and now and then inline
    /// markup: emphasis, strong, code, a key, math.
    fn sentence(&mut self, count: usize, out: &mut impl fmt::Write) -> fmt::Result {
        let mut first = true;
        for n in 0..count {
            if n > 0 {
                out.write_char(' ')?;
            }
            let word = self.word();
            match self.below(40) {
                0 => write!(out, "*{word}*")?,
                1 => write!(out, "**{word}**")?,
                2 => write!(out, "`{word}()`")?,
                3 => out.write_str("<kbd>Ctrl</kbd>")?,
                4 => out.write_str("$x^2 + y_i$")?,
                5 => write!(out, "==({word})==")?,
                _ if first => {
                    let mut chars = word.chars();
                    if let Some(initial) = chars.next() {
                        out.write_char(initial.to_ascii_uppercase())?;
                    }
                    out.write_str(chars.as_str())?;
                }
                _ => out.write_str(word)?,
            }
            first = false;
        }
        out.write_char('.')
    }

    /// A paragraph of `sentences` sentences.
    fn paragraph(&mut self, sentences: usize, out: &mut impl fmt::Write) -> fmt::Result {
        for n in 0..sentences {
            if n > 0 {
                out.write_char(' ')?;
            }
            let count = 8 + self.below(12);
            self.sentence(count, out)?;
        }
        out.write_str("\n\n")
    }
}

/// Code in three languages, language then text, as a post quotes it.
pub const CODE: &[(&str, &str)] = &[
    (
        "rust",
        "/// A page and the route it is served at.\n\
         #[derive(Debug, Clone)]\n\
         pub struct Page<'a> {\n    route: &'a str,\n    words: usize,\n}\n\n\
         impl<'a> Page<'a> {\n    pub fn new(route: &'a str) -> Self {\n        Self { route, words: 0 }\n    }\n\n\
         \x20   /// Minutes to read, rounded up.\n    pub fn minutes(&self) -> usize {\n        self.words.div_ceil(220)\n    }\n}\n\n\
         fn main() {\n    let pages: Vec<Page<'_>> = [\"/\", \"/posts/one\"].into_iter().map(Page::new).collect();\n\
         \x20   for page in &pages {\n        println!(\"{} takes {} min\", page.route, page.minutes());\n    }\n}\n",
    ),
    (
        "python",
        "import re\nfrom dataclasses import dataclass\n\n\n@dataclass\nclass Page:\n    \"\"\"A page and its words.\"\"\"\n\n\
         \x20   route: str\n    words: int = 0\n\n    def minutes(self) -> int:\n        # rounded up, like the theme\n\
         \x20       return -(-self.words // 220)\n\n\ndef slug(text: str) -> str:\n\
         \x20   return re.sub(r\"[^a-z0-9]+\", \"-\", text.lower()).strip(\"-\")\n\n\n\
         if __name__ == \"__main__\":\n    for page in [Page(\"/\", 900), Page(\"/posts/one\", 2400)]:\n\
         \x20       print(f\"{page.route}: {page.minutes()} min, {slug(page.route)!r}\")\n",
    ),
    (
        "typescript",
        "interface Page {\n  route: string;\n  words: number;\n  tags?: readonly string[];\n}\n\n\
         export function minutes(page: Page): number {\n  // rounded up, like the theme\n  return Math.ceil(page.words / 220);\n}\n\n\
         const pages: Page[] = [\n  { route: \"/\", words: 900 },\n  { route: \"/posts/one\", words: 2400, tags: [\"rust\"] },\n];\n\n\
         for (const page of pages) {\n  const tags = page.tags?.join(\", \") ?? \"none\";\n\
         \x20 console.log(`${page.route}: ${minutes(page)} min (${tags})`);\n}\n",
    ),
];

/// One post of about 2,000 words, the seed choosing its words. With `links` above zero it links
/// to `post-<n>.md` for a few `n` below `links`, as a post of a [`Blog`] does.
#[derive(Clone, Copy, Debug)]
pub struct Post {
    /// What chooses its words.
    pub seed: u64,
    /// How many posts sit beside it to link to; zero for none.
    pub links: usize,
}

impl Post {
    /// The sections a post has, each about 230 words.
    const SECTIONS: usize = 8;

    /// A paragraph with a link to another post and one outside, and a footnote reference.
    fn linked(&self, section: usize, draws: &mut Draws, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        draws.words(6, out)?;
        if self.links > 0 {
            let other = draws.below(self.links);
            write!(out, " [the {} post](post-{other}.md)", draws.word())?;
        }
        write!(out, " and [an outside page](https://example.com/{}/{section})", draws.word())?;
        write!(out, " with a note[^n{section}]. ")?;
        draws.paragraph(2, out)
    }

    /// A section: a heading, prose, then one block kind or two, cycling through every kind.
    fn section(&self, section: usize, draws: &mut Draws, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("## ")?;
        let count = 3 + draws.below(3);
        draws.words(count, out)?;
        out.write_str("\n\n")?;
        draws.paragraph(3, out)?;
        self.linked(section, draws, out)?;
        Self::blocks(section, draws, out)?;
        if let Some((language, code)) = CODE.get(section % CODE.len()) {
            write!(out, "```{language}\n{code}```\n\n")?;
        }
        draws.paragraph(3, out)?;
        draws.paragraph(2, out)
    }

    /// The blocks of a section beside its prose: lists, a table, math and a callout, or folded
    /// HTML and a definition, by the section's number.
    fn blocks(section: usize, draws: &mut Draws, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match section % 4 {
            0 => {
                for n in 0..6 {
                    out.write_str("- ")?;
                    draws.sentence(6 + n, out)?;
                    out.write_char('\n')?;
                }
                out.write_str("\n1. first\n2. second with `code`\n3. third\n\n")?;
                out.write_str("- [x] done\n- [ ] open\n\n")?;
            }
            1 => {
                out.write_str("| Name | Words | Note |\n|:-----|------:|:----:|\n")?;
                for _ in 0..8 {
                    out.write_str("| ")?;
                    draws.words(2, out)?;
                    write!(out, " | {} | ", draws.below(5_000))?;
                    draws.words(3, out)?;
                    out.write_str(" |\n")?;
                }
                out.write_char('\n')?;
            }
            2 => {
                out.write_str("$$\n\\int_0^1 x^2 \\, dx = \\frac{1}{3}\n$$\n\n")?;
                out.write_str("> [!note]\n> ")?;
                draws.sentence(12, out)?;
                out.write_str("\n\n> ")?;
                draws.sentence(14, out)?;
                out.write_str("\n\n")?;
            }
            _ => {
                out.write_str("<details>\n<summary>More</summary>\n\n")?;
                draws.paragraph(1, out)?;
                out.write_str("</details>\n\nTerm\n: ")?;
                draws.sentence(9, out)?;
                out.write_str("\n\n")?;
            }
        }
        Ok(())
    }
}

impl fmt::Display for Post {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut draws = Draws::new(self.seed);
        let day = 1 + self.seed % 28;
        let month = 1 + (self.seed / 28) % 12;
        out.write_str("---\ntitle: ")?;
        draws.words(4, out)?;
        write!(
            out,
            " {}\ndate: 2026-{month:02}-{day:02}\ntags: [{}, {}]\ndescription: ",
            self.seed,
            draws.word(),
            draws.word()
        )?;
        draws.words(12, out)?;
        out.write_str("\n---\n\n")?;
        draws.paragraph(4, out)?;
        for section in 0..Self::SECTIONS {
            self.section(section, &mut draws, out)?;
        }
        for section in 0..Self::SECTIONS {
            write!(out, "[^n{section}]: ")?;
            draws.sentence(10, out)?;
            out.write_str("\n\n")?;
        }
        Ok(())
    }
}

/// A body of `paragraphs` short paragraphs, about 80 bytes each, nothing else.
#[derive(Clone, Copy, Debug)]
pub struct Flood {
    /// How many paragraphs.
    pub paragraphs: usize,
}

impl fmt::Display for Flood {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut draws = Draws::new(u64::try_from(self.paragraphs).unwrap_or(0));
        for _ in 0..self.paragraphs {
            draws.sentence(13, out)?;
            out.write_str("\n\n")?;
        }
        Ok(())
    }
}

/// `block` written `times` times, one after the other.
#[derive(Clone, Copy, Debug)]
pub struct Repeated {
    /// What is written, with the blank line that ends it.
    pub block: &'static str,
    /// How many times.
    pub times: usize,
}

impl fmt::Display for Repeated {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        (0..self.times).try_for_each(|_| out.write_str(self.block))
    }
}

/// `count` paragraphs each referencing a footnote of its own, then the footnotes.
#[derive(Clone, Copy, Debug)]
pub struct Notes {
    /// How many footnotes.
    pub count: usize,
}

impl fmt::Display for Notes {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        (0..self.count).try_for_each(|n| write!(out, "A claim[^{n}].\n\n"))?;
        (0..self.count).try_for_each(|n| write!(out, "[^{n}]: Its source, {n}.\n\n"))
    }
}

/// A blog of `posts` posts under `posts/`, each linking to others, and an about page.
#[derive(Clone, Copy, Debug)]
pub struct Blog {
    /// How many posts.
    pub posts: usize,
}

impl Blog {
    /// Every file, path then text, in path order.
    #[must_use]
    pub fn files(&self) -> Vec<(String, String)> {
        let mut files: Vec<(String, String)> = (0..self.posts)
            .map(|n| {
                let post = Post { seed: n as u64, links: self.posts };
                (format!("posts/post-{n}.md"), post.to_string())
            })
            .collect();
        let mut about = String::from("---\ntitle: About\n---\n\n");
        let mut draws = Draws::new(u64::MAX);
        let _infallible = draws.paragraph(5, &mut about);
        files.push(("about.md".to_owned(), about));
        files.sort();
        files
    }
}
