//! The dashboard's one stylesheet: the doodle look (white paper, black pencil, one red pen, bucket
//! fills offset from their lines) from the shared tokens. Everything is visible without motion;
//! the one animation is behind `prefers-reduced-motion: no-preference`.

use token::Colour;
use unspliced::css::Color;

/// A token colour as the stylesheet writes it, so a colour exists once, in `token/token.toml`.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Paint(pub(crate) Colour);

impl Paint {
    /// The colour as a stylesheet value.
    pub(crate) const fn color(self) -> Color {
        let token::Rgb(red, green, blue) = self.0.rgb();
        Color::hex(((red as u32) << 16) | ((green as u32) << 8) | blue as u32)
    }
}

const INK: Color = Paint(Colour::Ink).color();
const PEN: Color = Paint(Colour::Pen).color();
const PENCIL: Color = Paint(Colour::Pencil).color();
const PAPER: Color = Paint(Colour::Paper).color();
const BUCKET: Color = Paint(Colour::Bucket).color();
const SKY: Color = Paint(Colour::Sky).color();
const MINT: Color = Paint(Colour::Mint).color();
const PINK: Color = Paint(Colour::Pink).color();
const CLOUD: Color = Paint(Colour::Cloud).color();

unspliced::css! {
    pub(crate) mod look;

    *, ::before, ::after { box-sizing: border-box; }
    body {
        margin: 0;
        background: (PAPER);
        color: (INK);
        font-family: "Gaegu", system-ui, sans-serif;
        font-size: 1.25rem;
        line-height: 1.5;
    }
    a { color: (INK); text-decoration-thickness: 2px; text-underline-offset: 3px; }
    a:hover { background: (BUCKET); }
    code, pre, kbd, .path { font-family: "Monaspace Radon", ui-monospace, monospace; font-size: 0.85 em; }
    pre { margin: 0; overflow-x: auto; }
    h1, h2, h3 { line-height: 1.15; margin: 0; font-weight: 700; }
    h1 { font-size: 2.4rem; }
    h2 { font-size: 1.7rem; margin: 2.5rem 0 0.75rem; }
    h3 { font-size: 1.3rem; margin: 1.5rem 0 0.5rem; }
    p { margin: 0.5rem 0; max-width: 60ch; }
    :focus-visible { outline: 3px solid (INK); outline-offset: 3px; background: (BUCKET); }
    .skip { position: absolute; left: 0.5rem; top: -4rem; padding: 0.5rem 1rem; background: (BUCKET); border: 3px solid (INK); }
    .skip:focus { top: 0.5rem; }

    .page { max-width: 62rem; margin: 0 auto; padding: 1rem 1rem 5rem; }
    .head { display: flex; flex-wrap: wrap; gap: 0.5rem 1.5rem; align-items: center; justify-content: space-between; padding-bottom: 0.75rem; }
    .brand { font-size: 1.6rem; font-weight: 700; text-decoration: none; color: (PEN); }
    .brand:hover { background: transparent; text-decoration: underline; }
    .nav { display: flex; flex-wrap: wrap; gap: 0.25rem 1rem; margin: 0; padding: 0; list-style: none; }
    .nav a { display: inline-block; padding: 0.25rem 0.5rem; text-decoration: none; border: 3px solid transparent; }
    .nav a[aria-current] { border-color: (INK); background: (SKY); }
    .who { color: (PENCIL); font-size: 1.1rem; }
    .rule { display: block; width: 100%; height: 14px; margin: 0.25rem 0 1.5rem; overflow: visible; }
    .rule path { fill: (INK); }
    .rule.red path { fill: (PEN); }

    .lede { color: (PENCIL); }
    .line { display: flex; flex-wrap: wrap; align-items: baseline; gap: 0.25rem 0.75rem; margin: 0.25rem 0; }
    .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(17rem, 1fr)); gap: 1rem 1.5rem; margin: 1rem 0; }
    .card { border: 3px solid (INK); padding: 0.75rem 1rem; background: (PAPER); }
    .card h2, .card h3 { margin-top: 0; }
    .fill-sky { background: (SKY); }
    .fill-mint { background: (MINT); }
    .fill-pink { background: (PINK); }
    .fill-cloud { background: (CLOUD); }
    .fill-bucket { background: (BUCKET); }

    .state { display: inline-block; padding: 0 0.6rem; border: 3px solid (INK); font-weight: 700; }
    .state.live { background: (MINT); }
    .state.paused { background: (BUCKET); }
    .state.none { background: (CLOUD); }
    .state.bad { background: (PINK); }
    .state.run { background: (SKY); }
    .glyph { font-weight: 700; margin-right: 0.35rem; }

    .button, button { font: inherit; font-weight: 700; color: (INK); background: (PAPER); border: 3px solid (INK); padding: 0.25rem 1rem; cursor: pointer; text-decoration: none; display: inline-block; }
    .button.main, button.main { background: (BUCKET); box-shadow: 5px 4px 0 (INK); }
    .button.danger, button.danger { background: (PINK); }
    .button:hover, button:hover { background: (BUCKET); }
    .stack { display: grid; gap: 0.75rem; max-width: 34rem; margin: 0.75rem 0; }
    label { font-weight: 700; display: block; }
    input[type="text"], input[type="search"], select, textarea { font: inherit; width: 100%; padding: 0.25rem 0.5rem; border: 3px solid (INK); background: (PAPER); color: (INK); }
    .hint { color: (PENCIL); font-size: 1.05rem; margin: 0.125rem 0 0; }

    .table { width: 100%; overflow-x: auto; margin: 0.5rem 0 1rem; }
    table { border-collapse: collapse; width: 100%; min-width: 34rem; }
    caption { text-align: left; font-weight: 700; font-size: 1.3rem; padding: 0.5rem 0; }
    th, td { text-align: left; vertical-align: top; padding: 0.35rem 0.6rem; border-bottom: 2px solid (INK); }
    th { background: (CLOUD); }
    tr.check td { background: (BUCKET); }

    details { border: 3px solid (INK); padding: 0.25rem 0.75rem; margin: 0.5rem 0; }
    details[open] { background: (CLOUD); }
    summary { cursor: pointer; font-weight: 700; padding: 0.125rem 0; }
    .ladder { margin: 0.5rem 0; padding-left: 1.5rem; }
    .ladder li { margin: 0.25rem 0; }
    .chosen { background: (MINT); padding: 0 0.25rem; }

    .note { color: (PEN); font-weight: 700; margin: 0.5rem 0; }
    .empty { border: 3px dashed (INK); padding: 1rem; margin: 1rem 0; max-width: 40rem; }
    .empty h2 { margin-top: 0; }
    .failure { border: 3px solid (PEN); padding: 0.75rem 1rem; margin: 1rem 0; max-width: 46rem; }
    .failure h2 { margin: 0 0 0.5rem; color: (PEN); }
    .failure dl { display: grid; grid-template-columns: max-content 1fr; gap: 0.25rem 1rem; margin: 0.5rem 0 0; }
    .failure dt { font-weight: 700; }
    .failure dd { margin: 0; }
    .failure ol { margin: 0; padding-left: 1.25rem; }
    .banner { border: 3px solid (INK); background: (BUCKET); padding: 0.5rem 1rem; margin: 1rem 0; }
    .log { background: (PAPER); border: 3px solid (INK); padding: 0.5rem 0.75rem; margin: 1rem 0; overflow-x: auto; }
    .log div { white-space: pre-wrap; overflow-wrap: anywhere; }
    .log .bad { color: (PEN); font-weight: 700; }
    .danger-zone { border: 3px solid (PEN); padding: 0.5rem 1rem; margin-top: 2rem; }
    .meta { color: (PENCIL); }
    .stamp { font-variant-numeric: tabular-nums; }

    @media (max-width: 40rem) {
        body { font-size: 1.15rem; }
        h1 { font-size: 2rem; }
        .failure dl { grid-template-columns: 1fr; }
        .button.main, button.main { box-shadow: 4px 3px 0 (INK); }
    }
    @media (prefers-reduced-motion: no-preference) {
        .state.run { animation: pulse 1.6s ease-in-out infinite; }
        @keyframes pulse { 0% { opacity: 1; } 50% { opacity: 0.6; } 100% { opacity: 1; } }
    }
}
