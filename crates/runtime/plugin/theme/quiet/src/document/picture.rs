//! A raster image as the engine prepared it: a `<picture>` offering AVIF and then the fallback
//! every browser shows, at each width, with `width` and `height` so the page does not move as it
//! loads; under it, in the same grid cell, its placeholder, which the image covers once loaded.
//! An image served as it is (an animated GIF) is a plain `<img>` with its size.

use std::fmt;

use model::{Asset, Dimensions, MediaType, Picture, Variant};
use unspliced::{Halt, Render, Sink, Url, html};

use crate::style::quiet;

/// How wide an image is shown: the page's measure (68ch, about 40rem) on a wide screen, the
/// screen less the page's padding on a narrow one.
const SIZES: &str = "(min-width: 42rem) 40rem, calc(100vw - 2rem)";

/// One prepared image, as the page shows it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Shown<'a> {
    pub(crate) asset: &'a Asset,
    pub(crate) picture: &'a Picture,
    pub(crate) alt: &'a str,
}

/// The copies of one format, as a `srcset`: `/a.480.<hash>.avif 480w, ...`.
#[derive(Debug)]
struct Srcset<'a>(Vec<&'a Variant>);

impl fmt::Display for Srcset<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (nth, variant) in self.0.iter().enumerate() {
            let separator = if nth == 0 { "" } else { ", " };
            write!(out, "{separator}{} {}w", variant.route, variant.dimensions.width)?;
        }
        Ok(())
    }
}

impl Render for Shown<'_> {
    fn render(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        let Dimensions { width, height, .. } = self.picture.dimensions;
        let alt = self.alt;
        let fallback: Vec<&Variant> =
            [MediaType::Jpeg, MediaType::Png].into_iter().flat_map(|media| self.picture.of(media)).collect();
        let Some(largest) = fallback.last() else {
            let Ok(src) = Url::checked(self.asset.route.as_str()) else { return alt.render(out) };
            return html! { img src=(src) alt=(alt) width=(width) height=(height) loading="lazy" decoding="async"; }
                .render(out);
        };
        let Ok(src) = Url::checked(largest.route.as_str()) else { return alt.render(out) };
        let avif = Srcset(self.picture.of(MediaType::Avif).collect()).to_string();
        let fallback = Srcset(fallback).to_string();
        let picture = html! {
            picture {
                @if !avif.is_empty() { source type="image/avif" srcset=(&avif) sizes=(SIZES); }
                img src=(src) srcset=(&fallback) sizes=(SIZES) width=(width) height=(height) alt=(alt) loading="lazy" decoding="async";
            }
        };
        let placeholder = self.picture.placeholder.as_ref().and_then(|route| Url::checked(route.as_str()).ok());
        match placeholder {
            Some(under) => html! {
                span.quiet::PICTURE { img src=(under) alt="" width=(width) height=(height); (picture) }
            }
            .render(out),
            None => picture.render(out),
        }
    }
}
