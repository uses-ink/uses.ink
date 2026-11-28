//! The theme's one stylesheet: system fonts, a readable measure, light and dark from the reader's
//! own setting. Deliberately plain; the real design is the owner's (open.md T1).
//!
//! Colours, faces and sizes are custom properties (`--ink`, `--face-body`, `--size-h1`), declared
//! here for the theme's own look and again, from `crates/core/token/token.toml`, by a look
//! (`look = "uses-ink"`, decision 0142) in a stylesheet that comes after this one.

unspliced::css! {
    pub(crate) mod quiet;

    :root {
        color-scheme: light dark;
        --ink: #1d1d1b;
        --paper: #fcfbf8;
        --muted: #6a675f;
        --line: #e2ded5;
        --tint: #f2efe8;
        --accent: #2b59c3;
        --face-body: system-ui, sans-serif;
        --face-mono: ui-monospace, monospace;
        --size-small: 0.875rem;
        --size-body: 1.0625rem;
        --size-h3: 1.17rem;
        --size-h2: 1.5rem;
        --size-h1: 2rem;
    }
    @media (prefers-color-scheme: dark) {
        :root {
            --ink: #e7e4dc;
            --paper: #171715;
            --muted: #a29e94;
            --line: #36342f;
            --tint: #222120;
            --accent: #8fb0ff;
        }
    }
    *, ::before, ::after { box-sizing: border-box; }
    body {
        margin: 0;
        background: var(--paper);
        color: var(--ink);
        font-family: var(--face-body);
        font-size: var(--size-body);
        line-height: 1.6;
    }
    .page { max-width: 68ch; margin: 0 auto; padding: 1.5rem 1rem 4rem; }
    .head { display: flex; flex-wrap: wrap; gap: 0.5rem 1.5rem; align-items: baseline; margin-bottom: 2.5rem; }
    .head ul { display: flex; flex-wrap: wrap; gap: 1rem; margin: 0; padding: 0; list-style: none; }
    .site { font-weight: 700; color: var(--ink); text-decoration: none; }
    a { color: var(--accent); }
    h1, h2, h3, h4, h5, h6 { line-height: 1.25; margin: 2rem 0 0.75rem; }
    h1 { font-size: var(--size-h1); margin-top: 0; }
    h2 { font-size: var(--size-h2); }
    h3 { font-size: var(--size-h3); }
    .anchor { margin-left: 0.4rem; color: var(--muted); text-decoration: none; visibility: hidden; }
    h2:hover .anchor, h3:hover .anchor, h4:hover .anchor, h5:hover .anchor, h6:hover .anchor { visibility: visible; }
    .meta { color: var(--muted); font-size: 0.9375rem; margin: -0.25rem 0 2rem; }
    .tags { display: inline; margin: 0 0 0 0.75rem; padding: 0; list-style: none; }
    .tags li { display: inline; margin-right: 0.5rem; }
    blockquote { margin: 1.5rem 0; padding: 0 1rem; border-left: 3px solid var(--line); color: var(--muted); }
    pre { overflow-x: auto; padding: 0.75rem 1rem; border-radius: 6px; background: var(--tint); font-size: var(--size-small); }
    code, kbd { font-family: var(--face-mono); font-size: 0.9 em; }
    kbd { padding: 0 0.3rem; border: 1px solid var(--line); border-radius: 4px; }
    mark { padding: 0 0.15rem; }
    img { max-width: 100%; height: auto; }
    .picture { display: inline-grid; vertical-align: top; }
    .picture > * { grid-row: 1; grid-column: 1; }
    hr { border: 0; border-top: 1px solid var(--line); margin: 2rem 0; }
    table { border-collapse: collapse; margin: 1.5rem 0; display: block; overflow-x: auto; }
    th, td { padding: 0.35rem 0.75rem; border-bottom: 1px solid var(--line); }
    .left { text-align: left; }
    .center { text-align: center; }
    .right { text-align: right; }
    .task { list-style: none; }
    .task input { margin: 0 0.5rem 0 -1.25rem; }
    .callout { margin: 1.5rem 0; padding: 0.75rem 1rem; border-left: 3px solid var(--accent); background: var(--tint); }
    .callout > :last-child { margin-bottom: 0; }
    .label { margin-top: 0; font-weight: 700; }
    summary.label { margin-bottom: 0.5rem; }
    .embed { margin: 1.5rem 0; padding-left: 1rem; border-left: 3px solid var(--line); }
    .embed > :last-child { margin-bottom: 0; }
    .warning, .caution { border-left-color: #c2410c; }
    .tip { border-left-color: #15803d; }
    details { margin: 1.5rem 0; }
    summary { cursor: pointer; font-weight: 600; }
    dt { font-weight: 600; }
    dd { margin: 0 0 0.75rem 1.5rem; }
    .math { overflow-x: auto; margin: 1.5rem 0; }
    .unparsed { color: #b91c1c; }
    .notes { margin-top: 3rem; padding-top: 1rem; border-top: 1px solid var(--line); font-size: 0.9375rem; }
    .back { margin-left: 0.25rem; text-decoration: none; }
    .listing { padding: 0; list-style: none; }
    .listing li { margin: 0 0 0.75rem; }
    .listing time { display: inline-block; min-width: 7rem; color: var(--muted); font-variant-numeric: tabular-nums; }
    .foot { margin-top: 4rem; color: var(--muted); font-size: var(--size-small); }

    .side, .outline { margin: 0 0 1.5rem; font-size: var(--size-small); }
    .side ul, .outline ul { margin: 0; padding: 0; list-style: none; }
    .side li, .outline li { margin: 0.15rem 0; }
    .side details { margin: 0.5rem 0; }
    .side summary { padding: 0.15rem 0.4rem; }
    .side details > ul { margin-left: 0.4rem; padding-left: 0.6rem; border-left: 1px solid var(--line); }
    .side a { display: block; padding: 0.15rem 0.4rem; border-radius: 4px; color: var(--ink); text-decoration: none; }
    .side a:hover { text-decoration: underline; }
    .side .current { background: var(--tint); font-weight: 700; }
    .rubric { margin: 0 0 0.4rem; color: var(--muted); font-weight: 600; }
    .deep { padding-left: 0.8rem; }
    .deeper { padding-left: 1.6rem; }
    .pager { display: flex; flex-wrap: wrap; gap: 1rem; margin-top: 3rem; padding-top: 1rem; border-top: 1px solid var(--line); }
    .pager a { display: block; flex: 1 1 14rem; padding: 0.5rem 0.75rem; border: 1px solid var(--line); border-radius: 6px; text-decoration: none; }
    .pager .before { text-align: left; }
    .pager .after { text-align: right; }
    .hint { display: block; color: var(--muted); font-size: var(--size-small); }
    .wide { max-width: 80rem; }
    @media (min-width: 64rem) {
        .wide { display: grid; grid-template-columns: 15rem minmax(0, 1fr) 13rem; column-gap: 2.5rem; align-items: start; }
        .wide .head, .wide .foot { grid-column: 1 / 4; }
        .side { grid-column: 1; grid-row: 2; position: sticky; top: 1rem; max-height: 100vh; overflow-y: auto; }
        .outline { grid-column: 3; grid-row: 2; position: sticky; top: 1rem; max-height: 100vh; overflow-y: auto; }
        .body { grid-column: 2; grid-row: 2; min-width: 0; }
    }
}

// The highlighting stylesheet: one class per capture of `model::highlight::Capture`, coloured
// after GitHub's light and dark themes, the dark one from the reader's setting. The mapping from
// capture to class is `document::code`'s `Shade`, so a capture without a class does not compile.
unspliced::css! {
    pub(crate) mod code;

    :root {
        --code-keyword: #cf222e;
        --code-function: #8250df;
        --code-string: #0a3069;
        --code-comment: #6e7781;
        --code-constant: #0550ae;
        --code-variable: #953800;
        --code-tag: #116329;
        --code-inserted: #116329;
        --code-deleted: #82071f;
    }
    @media (prefers-color-scheme: dark) {
        :root {
            --code-keyword: #ff7b72;
            --code-function: #d2a8ff;
            --code-string: #a5d6ff;
            --code-comment: #8b949e;
            --code-constant: #79c0ff;
            --code-variable: #ffa657;
            --code-tag: #7fe787;
            --code-inserted: #aff5b4;
            --code-deleted: #ffdcd7;
        }
    }
    .keyword, .label { color: var(--code-keyword); }
    .function, .macro, .constructor, .attribute, .module { color: var(--code-function); }
    .string, .literal, .link { color: var(--code-string); }
    .comment { color: var(--code-comment); font-style: italic; }
    .constant, .number, .type, .property, .heading { color: var(--code-constant); }
    .variable { color: var(--code-variable); }
    .tag { color: var(--code-tag); }
    .inserted { color: var(--code-inserted); }
    .deleted, .error { color: var(--code-deleted); }
    .heading, .strong { font-weight: 700; }
    .emphasis { font-style: italic; }
    .strike { text-decoration: line-through; }
    .operator, .punctuation, .embedded { color: inherit; }
}
