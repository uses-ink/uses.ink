//! The dashboard's one stylesheet (decision 0190): the doodle binder. White paper, a black
//! pencil of one even width, bucket fills that miss their lines by 5 px right and 4 px down, and
//! the red pen only where uses.ink speaks. Every colour and face is a custom property of
//! `token.css`, so a colour exists once, in `crates/core/token/token.toml`.
//!
//! A box is four crossing strokes drawn by `freehand` (`/outline-1.svg`), laid on an element's
//! `::after` as a border image, so a line keeps its width at any size. Where the image does not
//! load, the same pseudo-element is a plain black 3 px box. A fill is the element's `::before`.
//! Everything is visible without motion; the one movement sits behind `prefers-reduced-motion:
//! no-preference`.

unspliced::css! {
    pub(crate) mod look;

    @font-face { font-family: "Gaegu"; src: url("/gaegu-regular.woff2") format("woff2"); font-weight: 400; font-display: swap; }
    @font-face { font-family: "Gaegu"; src: url("/gaegu-bold.woff2") format("woff2"); font-weight: 700; font-display: swap; }
    @font-face { font-family: "Monaspace Radon"; src: url("/monaspace-radon.woff2") format("woff2"); font-weight: 400; font-display: swap; }

    *, ::before, ::after { box-sizing: border-box; }
    * { margin: 0; }
    html { background: var(--paper); color: var(--ink); }
    body {
        font-family: var(--hand);
        font-size: 1.3125rem;
        font-weight: 400;
        line-height: 1.4;
        letter-spacing: -0.04 em;
        word-spacing: -0.1 em;
        font-synthesis: none;
    }
    ::selection { background: var(--bucket); }
    a { color: var(--ink); text-decoration-thickness: 2px; text-underline-offset: 3px; }
    a:hover { background: var(--bucket); }
    code, pre, kbd, .path { font-family: var(--mono); font-size: 0.8 em; letter-spacing: 0; word-spacing: 0; overflow-wrap: anywhere; }
    pre { overflow-x: auto; white-space: pre-wrap; overflow-wrap: anywhere; }
    b, strong, h1, h2, h3, th, summary, label { font-weight: 700; }
    h1 { font-size: 3.25rem; line-height: 1; letter-spacing: -0.05 em; text-transform: lowercase; }
    h2 { font-size: 2.125rem; line-height: 1.1; letter-spacing: -0.05 em; margin-top: 3.25rem; text-transform: lowercase; }
    h3 { font-size: 1.5rem; line-height: 1.15; margin-top: 1.25rem; text-transform: lowercase; }
    h2 + *, h3 + * { margin-top: 0.75rem; }
    p { max-width: 64ch; margin-top: 0.5rem; }
    ul, ol { padding-left: 1.5rem; margin-top: 0.5rem; }
    li { margin-top: 0.25rem; }
    :focus-visible { outline: 3px dashed var(--pen); outline-offset: 4px; border-radius: 4px; }
    .skip { position: absolute; left: 0.5rem; top: -6rem; z-index: 9; padding: 0.5rem 1rem; background: var(--bucket); border: 3px solid var(--ink); }
    .skip:focus { top: 0.5rem; }

    /* the binder: tabs down the left, one sheet on the right */
    .app { display: grid; grid-template-columns: 14.5rem minmax(0, 1fr); min-height: 100vh; }
    .rail { position: sticky; top: 0; height: 100vh; z-index: 2; padding: 1.625rem 0 1.625rem 1.5rem; display: flex; flex-direction: column; gap: 0.75rem; }
    .brand { display: flex; align-items: center; gap: 0.375rem; font-size: 2.125rem; font-weight: 700; line-height: 1; letter-spacing: -0.04 em; margin: 0 0 1.5rem 0.5rem; text-decoration: none; }
    .brand:hover { background: transparent; }
    .brand img { width: 2.125rem; height: 2.125rem; }
    .pick { margin: 0 1.25rem 1rem 0.5rem; font-size: 1.1875rem; color: var(--pencil); }
    .pick b { display: block; color: var(--ink); font-size: 1.3125rem; }
    .rail nav { flex: 1; display: flex; flex-direction: column; }
    .nav { list-style: none; padding: 0; margin: 0; flex: 1; display: flex; flex-direction: column; gap: 0.75rem; }
    .nav li { margin: 0; }
    .tab { display: block; padding: 0.75rem 1.125rem; margin-right: -0.25rem; font-size: 1.4375rem; font-weight: 700; text-decoration: none; }
    .tab:hover { background: transparent; }
    .tab, .open, .button, button, th, caption, summary { text-transform: lowercase; }
    .tab[aria-current="page"] { margin-right: -1.875rem; padding-right: 3rem; z-index: 2; }
    .nav li:nth-child(1) .tab { --fill: var(--bucket); }
    .nav li:nth-child(2) .tab { --fill: var(--pink); }
    .nav li:nth-child(3) .tab { --fill: var(--sky); }
    .nav li:nth-child(4) .tab { --fill: var(--mint); }
    .nav li.me { margin-top: auto; }
    .nav li.me .tab { display: flex; align-items: center; gap: 0.625rem; font-size: 1.25rem; font-weight: 400; margin-right: 1.5rem; padding: 0.5rem; --fill: transparent; }
    .nav li.me .tab[aria-current="page"] { margin-right: 0; font-weight: 700; --fill: var(--cloud); }
    .me img { width: 2.375rem; height: 2.375rem; }
    .page { margin: 1.375rem 1.75rem 1.75rem 0; padding: 2.75rem 3.5rem 4rem; background: var(--paper); z-index: 1; min-width: 0; }
    .page > h1 + .lede { margin-top: 0.5rem; }

    /* boxes: four crossing strokes on ::after, a bucket fill on ::before */
    .page, .tab, .button, button, summary, details, .card, .item, .banner, .empty, .failure, .log, div.danger, .record, .dep, .open {
        position: relative;
        isolation: isolate;
    }
    .page::after, .tab::after, .button::after, button::after, details::after, .card::after, .item::after, .banner::after, .empty::after, .failure::after, .log::after, div.danger::after, .record::after, .dep::after, .open::after {
        content: "";
        position: absolute;
        inset: 0;
        z-index: 3;
        pointer-events: none;
        border: 3px solid var(--ink);
        border-image-source: url("/outline-1.svg");
        border-image-slice: 12;
        border-image-width: 12px;
        border-image-outset: 9px;
        border-image-repeat: stretch;
    }
    .card:nth-of-type(2n)::after, .dep:nth-child(2n)::after, .item:nth-of-type(2n)::after, details:nth-of-type(2n)::after { border-image-source: url("/outline-2.svg"); }
    .card:nth-of-type(3n)::after, .dep:nth-child(3n)::after, .item:nth-of-type(3n)::after, details:nth-of-type(3n)::after { border-image-source: url("/outline-3.svg"); }
    .tab::after, .button::after, button::after, .open::after { border-image-source: url("/outline-small.svg"); }
    .nav li.me .tab:not([aria-current="page"])::after { content: none; }
    .tab::before, .open::before, .banner::before, .record::before, .main::before, button.danger::before, .button:hover::before, button:hover::before {
        content: "";
        position: absolute;
        inset: 0;
        z-index: -1;
        translate: 5px 4px;
        background: var(--fill, var(--bucket));
    }

    .open { position: absolute; top: 2.75rem; right: 3.5rem; z-index: 3; padding: 0.625rem 1rem; font-size: 1.25rem; font-weight: 700; text-decoration: none; --fill: var(--mint); }
    .open:hover { background: transparent; }

    /* input fields wear the box as their own border image: they cannot hold a pseudo-element */
    input[type="text"], input[type="search"], input[type="email"], select, textarea {
        font: inherit;
        letter-spacing: inherit;
        word-spacing: inherit;
        width: 100%;
        padding: 0.375rem 0.75rem;
        color: var(--ink);
        background: var(--paper);
        border: 3px solid var(--ink);
        border-radius: 0;
        border-image-source: url("/outline-small.svg");
        border-image-slice: 12;
        border-image-width: 12px;
        border-image-outset: 9px;
        border-image-repeat: stretch;
    }
    input[type="radio"], input[type="checkbox"] { accent-color: var(--pen); width: 1.125rem; height: 1.125rem; translate: 0 3px; }
    label { display: block; }
    label:has(input[type="radio"]), label:has(input[type="checkbox"]) { display: flex; gap: 0.25rem 0.75rem; align-items: baseline; }
    .stack { display: grid; gap: 1.25rem; max-width: 34rem; margin: 1.25rem 0 0.75rem; }
    .stack > li { margin: 0; }
    ul.stack { list-style: none; padding: 0; max-width: none; }
    .button, button { font: inherit; font-weight: 700; letter-spacing: -0.04 em; color: var(--ink); background: transparent; border: 0; padding: 0.4375rem 1.25rem; cursor: pointer; text-decoration: none; display: inline-block; margin: 0.375rem 0.5rem 0.375rem 0; }
    .button:hover, button:hover { background: transparent; }
    .fill-mint::before { content: ""; position: absolute; inset: 0; z-index: -1; translate: 5px 4px; background: var(--mint); }
    button.danger::before { background: var(--pink); }
    button:disabled { color: var(--pencil); cursor: not-allowed; }
    .stack > button, .stack > .button { justify-self: start; }
    form { margin-top: 0.5rem; }
    .card + form, .card + .stack { margin-top: 1.75rem; }
    form .stack { margin-top: 0.75rem; }

    /* what uses.ink says */
    .lede { color: var(--pencil); font-size: 1.1875rem; }
    .lede code { color: var(--ink); }
    .meta, .hint { color: var(--pencil); }
    .hint { font-size: 1.0625rem; margin-top: 0.125rem; }
    .note { color: var(--pen); font-weight: 700; }
    .why { color: var(--pen); }
    .stamp { font-variant-numeric: tabular-nums; }
    .line { display: flex; flex-wrap: wrap; align-items: baseline; gap: 0.25rem 0.75rem; max-width: none; }
    .glyph { font-weight: 700; margin-right: 0.35rem; }
    .rule { display: block; width: 100%; height: 14px; overflow: visible; }
    .rule path { fill: var(--ink); }
    .rule.red path { fill: var(--pen); }
    .more { background-image: url("/underline.svg"); background-repeat: no-repeat; background-position: 0 100%; background-size: 100% 5px; padding-bottom: 2px; text-decoration: none; }

    /* a state is a word and a mark, never a colour alone */
    .state { display: inline-block; padding: 0 0.625rem; border-radius: 999px; border: 2px solid var(--ink); font-weight: 700; font-size: 1.125rem; line-height: 1.4; background: var(--paper); color: var(--pencil); }
    .state.live { background: var(--mint); color: var(--ink); }
    .state.paused { background: var(--bucket); color: var(--ink); }
    .state.none { background: var(--paper); color: var(--pencil); border-color: var(--pencil); }
    .state.bad { background: var(--paper); color: var(--pen); border-color: var(--pen); }
    .state.run { background: var(--sky); color: var(--ink); }

    /* cards and the lists of things that need a look */
    .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(17rem, 1fr)); gap: 1.5rem 1.75rem; margin-top: 1.5rem; }
    .grid > section { margin-top: 0; }
    .card { padding: 1.125rem 1.5rem; margin-top: 1.5rem; background: transparent; }
    .card > :first-child { margin-top: 0; }
    .card h2, .card h3 { margin-top: 0; }
    .item { padding: 1.125rem 1.375rem; margin-top: 1.125rem; }
    .banner { padding: 0.875rem 1.25rem; margin-top: 1.5rem; max-width: 46rem; }
    .banner > :first-child { margin-top: 0; }
    .empty { padding: 1.125rem 1.5rem; margin-top: 1.5rem; max-width: 44rem; }
    .empty h2 { margin-top: 0; font-size: 1.75rem; }
    .failure { padding: 1.125rem 1.5rem; margin-top: 1.5rem; max-width: 48rem; }
    .failure h2 { margin: 0 0 0.5rem; color: var(--pen); font-size: 1.75rem; text-transform: none; }
    .failure dl { display: grid; grid-template-columns: max-content 1fr; gap: 0.25rem 1rem; margin-top: 0.5rem; }
    .failure dt { font-weight: 700; color: var(--pencil); }
    .failure dd { margin: 0; }
    .failure dd:last-child, .failure dt + dd { overflow-wrap: anywhere; }
    .failure ol { margin: 0; padding-left: 1.25rem; }
    .danger { margin-top: 1.25rem; }
    div.danger { padding: 0.75rem 1.25rem; max-width: 40rem; }
    .log { padding: 1rem 1.25rem; margin-top: 1.25rem; font-size: 0.875rem; line-height: 1.7; overflow-x: auto; }
    .log div { white-space: pre-wrap; overflow-wrap: anywhere; font-family: var(--mono); font-size: 0.875rem; letter-spacing: 0; word-spacing: 0; }
    .bad { color: var(--pen); font-weight: 700; }
    .state.bad { font-size: 1.125rem; }
    .record { margin-top: 1.25rem; padding: 0.875rem 1.25rem; max-width: 100%; }
    details { margin-top: 1rem; padding: 0.375rem 1rem; }
    details > summary { cursor: pointer; padding: 0.25rem 0; margin: 0; background: transparent; display: list-item; }
    details > summary::after { content: none; }
    details > summary::before { content: none; }
    details[open] { padding-bottom: 1rem; }
    .ladder { margin-top: 0.75rem; }
    .chosen { background: var(--mint); padding: 0 0.25rem; }

    .item { display: grid; grid-template-columns: auto 1fr; gap: 0.25rem 1.125rem; align-items: center; }
    .item > div > :first-child { margin-top: 0; }
    .item p { max-width: none; }
    .ring { display: grid; place-items: center; width: 2.75rem; height: 2.75rem; border: 3px solid var(--pen); border-radius: 50%; color: var(--pen); font-weight: 700; font-size: 1.5rem; line-height: 1; }
    .flag { display: inline-block; padding: 0.125rem 0.875rem; border: 3px solid var(--pen); border-radius: 50%; color: var(--pen); font-weight: 700; }
    .hello { display: grid; grid-template-columns: auto 1fr; gap: 1.75rem; align-items: center; margin-top: 1.75rem; }
    .hello img { width: 6.875rem; height: auto; }
    .big { font-size: 2rem; line-height: 1.1; font-weight: 700; letter-spacing: -0.05 em; margin: 0 0 0.375rem; max-width: none; }
    code.big { font-size: 1.25rem; font-weight: 400; letter-spacing: 0; margin: 0; }
    .ticks { list-style: none; padding: 0; display: grid; gap: 0.625rem; margin-top: 0.875rem; }
    .ticks li { display: flex; gap: 0.75rem; align-items: baseline; margin: 0; }
    .tick { flex: none; width: 0.5rem; height: 1rem; border: solid var(--pen); border-width: 0 3px 3px 0; rotate: 40deg; translate: 0.25rem 0.125rem; margin-right: 0.375rem; }
    .dash { flex: none; width: 1.375rem; text-align: center; font-weight: 700; }
    .deploys { list-style: none; padding: 0; margin-top: 2rem; display: grid; gap: 1.5rem; }
    .deploys > li { margin: 0; }
    .dep { padding: 1.25rem 1.625rem; }
    .head { display: flex; flex-wrap: wrap; align-items: center; gap: 0.5rem 1rem; max-width: none; }
    .head form { margin: 0 0 0 auto; }
    .dep p { max-width: none; }
    .record { display: flex; flex-wrap: wrap; gap: 0.375rem 1.75rem; width: max-content; }
    .record dt { color: var(--pencil); font-size: 1.0625rem; font-weight: 400; }
    .record dd { margin: 0; overflow-wrap: anywhere; }
    .record div { max-width: 100%; }
    pre { margin-top: 0.75rem; color: var(--pencil); }

    /* tables, ruled in pencil; a wide one scrolls in a region that can be reached by keyboard */
    .table { width: 100%; overflow-x: auto; margin-top: 1rem; }
    table { width: 100%; border-collapse: collapse; font-size: 1.25rem; min-width: 34rem; }
    caption { text-align: left; font-weight: 700; font-size: 1.1875rem; color: var(--pencil); padding: 0.375rem 0; }
    th { text-align: left; color: var(--pencil); font-size: 1.125rem; padding: 0.375rem 0.875rem 0.375rem 0; }
    td { text-align: left; vertical-align: baseline; padding: 0.75rem 0.875rem 0.75rem 0; }
    tbody tr { background-image: url("/rule.svg"); background-repeat: no-repeat; background-position: 0 100%; background-size: 100% 6px; }
    tr.check td { color: var(--pen); }
    td details { border: 0; padding: 0; margin: 0; }
    td details::after { content: none; }
    td details[open] { background: transparent; }

    @media (max-width: 40rem) {
        .table { overflow-x: visible; }
        table, tbody, tr { display: block; min-width: 0; }
        thead { position: absolute; width: 1px; height: 1px; overflow: hidden; clip-path: inset(50%); }
        caption { display: block; }
        tr { display: flex; flex-wrap: wrap; gap: 0 1rem; padding: 0.5rem 0 0.25rem; }
        td, th[scope="row"] { display: block; padding: 0.125rem 0; max-width: 100%; overflow-wrap: anywhere; }
    }
    @media (max-width: 61.25rem) {
        .app { display: block; }
        .rail { position: static; height: auto; padding: 1rem 1rem 0; flex-direction: column; gap: 0.5rem; }
        .brand { margin: 0 0 0.5rem; font-size: 1.75rem; }
        .pick { display: none; }
        .rail nav { display: block; }
        .nav { flex-direction: row; flex-wrap: wrap; gap: 0.75rem 1rem; }
        .nav li.me { margin-top: 0; }
        .tab { font-size: 1.1875rem; padding: 0.5rem 0.75rem; margin-right: 0; }
        .tab[aria-current="page"] { margin-right: 0; padding-right: 0.75rem; }
        .nav li.me .tab { margin-right: 0; font-size: 1.1875rem; }
        .me img { width: 1.75rem; height: 1.75rem; }
        .page { margin: 1rem 1rem 1.5rem; padding: 1.625rem 1.25rem 2.5rem; }
        h1 { font-size: 2.375rem; }
        h2 { font-size: 1.875rem; margin-top: 2.5rem; }
        .item, .card { padding: 1rem; }
        .hello { grid-template-columns: 1fr; }
        .hello img { width: 5rem; }
        .open { position: relative; top: auto; right: auto; display: inline-block; margin-top: 1rem; }
        .record { width: auto; flex-direction: column; }
        .head form { margin-left: 0; }
        .failure dl { grid-template-columns: 1fr; }
    }
    @media (prefers-reduced-motion: no-preference) {
        .tab { transition: transform 0.12s steps(2, end); }
        .tab:hover { transform: translateX(4px); }
        .state.run { animation: pulse 1.6s steps(4, end) infinite; }
        @keyframes pulse { 0% { opacity: 1; } 50% { opacity: 0.6; } 100% { opacity: 1; } }
    }
}
