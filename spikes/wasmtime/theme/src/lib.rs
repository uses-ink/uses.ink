//! A theme that renders one page: the title underlined by a `pen.stroke` sized by `text.measure`.

use unspliced::{Render, html};

wit_bindgen::generate!({
    path: "../wit",
    world: "ink:theme/theme",
    // Packages from `deps/` are generated here too; a real theme would map them to a shared crate.
    generate_all,
});

use exports::ink::theme::probe;
use ink::host::pen::{self, Point};
use ink::host::text;

const FONT: &str = "serif";
const SIZE: f64 = 32.0;

struct Theme;

impl Guest for Theme {
    fn render(page: Page) -> String {
        let extent = text::measure(FONT, SIZE, &page.title);
        // unspliced renders no floats, so the box is whole pixels.
        let width = (extent.width + 8.0).ceil() as u32;
        let height = 12_u32;
        let underline = pen::stroke(Point { x: 4.0, y: 6.0 }, Point { x: extent.width + 4.0, y: 6.0 }, 2.0, &page.title);
        let view = format!("0 0 {width} {height}");
        let paragraphs: Vec<&str> = page.paragraphs.iter().map(String::as_str).collect();
        let title = page.title.as_str();
        html! {
            html {
                head { title { (title) } }
                body {
                    main {
                        h1 { (title) }
                        svg viewBox=(view) width=(width) height=(height) { path d=(underline.as_str()); }
                        @for p in &paragraphs { p { (*p) } }
                    }
                }
            }
        }
        .html()
    }
}

impl probe::Guest for Theme {
    fn measure(count: u32) -> f64 {
        (0..count).map(|_| text::measure(FONT, SIZE, "hello").width).sum()
    }

    fn stroke(count: u32) -> u64 {
        (0..count)
            .map(|i| {
                let to = Point { x: 100.0 + f64::from(i % 7), y: 0.0 };
                pen::stroke(Point { x: 0.0, y: 0.0 }, to, 2.0, "probe").len() as u64
            })
            .sum()
    }

    fn nothing() {}
}

export!(Theme);
