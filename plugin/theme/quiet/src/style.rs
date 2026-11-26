//! The theme's one stylesheet: system fonts, a readable measure, light and dark from the reader's
//! own setting. Deliberately plain; the real design is the owner's (open.md T1).

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
        font-family: system-ui, sans-serif;
        font-size: 1.0625rem;
        line-height: 1.6;
    }
    .page { max-width: 68ch; margin: 0 auto; padding: 1.5rem 1rem 4rem; }
    .head { display: flex; flex-wrap: wrap; gap: 0.5rem 1.5rem; align-items: baseline; margin-bottom: 2.5rem; }
    .head ul { display: flex; flex-wrap: wrap; gap: 1rem; margin: 0; padding: 0; list-style: none; }
    .site { font-weight: 700; color: var(--ink); text-decoration: none; }
    a { color: var(--accent); }
    h1, h2, h3, h4, h5, h6 { line-height: 1.25; margin: 2rem 0 0.75rem; }
    h1 { font-size: 2rem; margin-top: 0; }
    .anchor { margin-left: 0.4rem; color: var(--muted); text-decoration: none; visibility: hidden; }
    h2:hover .anchor, h3:hover .anchor, h4:hover .anchor, h5:hover .anchor, h6:hover .anchor { visibility: visible; }
    .meta { color: var(--muted); font-size: 0.9375rem; margin: -0.25rem 0 2rem; }
    .tags { display: inline; margin: 0 0 0 0.75rem; padding: 0; list-style: none; }
    .tags li { display: inline; margin-right: 0.5rem; }
    blockquote { margin: 1.5rem 0; padding: 0 1rem; border-left: 3px solid var(--line); color: var(--muted); }
    pre { overflow-x: auto; padding: 0.75rem 1rem; border-radius: 6px; background: var(--tint); font-size: 0.875rem; }
    code, kbd { font-family: ui-monospace, monospace; font-size: 0.9 em; }
    kbd { padding: 0 0.3rem; border: 1px solid var(--line); border-radius: 4px; }
    mark { padding: 0 0.15rem; }
    img { max-width: 100%; height: auto; }
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
    .foot { margin-top: 4rem; color: var(--muted); font-size: 0.875rem; }
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
