//! The page around every screen (decision 0190): the head, the skip link, the rail of tabs with
//! the mark and the person's own tab, and the sheet holding the one `<h1>` and the screen.

use std::fmt;

use control::Handle;
use unspliced::{Halt, Lazy, Render, Sink, Url, html};

use crate::asset::Asset;
use crate::escaped::Escaped;
use crate::route::{Path, Screen};
use crate::style::look;

/// Where a page sits: which site it is about, if any, and which screen it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    /// The site the page is about; none for Account and for failures that name no site.
    pub site: Option<Handle>,
    /// The screen it belongs to; none for a failure that belongs to no screen.
    pub here: Option<Screen>,
}

impl Frame {
    /// The frame of a screen of the site `handle`.
    pub fn of(handle: &Handle, here: Screen) -> Self {
        Self { site: Some(handle.clone()), here: Some(here) }
    }

    /// The frame of the Account screen.
    pub const fn account() -> Self {
        Self { site: None, here: Some(Screen::Account) }
    }
}

/// A page's content: everything a screen shows is a function of its view model, written here.
pub trait View {
    /// Where the page sits.
    fn frame(&self) -> &Frame;

    /// The page's one heading, which is also its title.
    fn title(&self) -> impl fmt::Display + '_;

    /// Whether the browser should ask again in a few seconds: only while a build runs.
    fn refresh(&self) -> bool {
        false
    }

    /// The content under the heading.
    ///
    /// # Errors
    /// [`Halt`] from the sink.
    fn body(&self, out: &mut dyn Sink) -> Result<(), Halt>;
}

/// A whole page: a view inside the shell.
#[derive(Clone, Copy, Debug)]
pub struct Document<'a, V: ?Sized>(pub &'a V);

impl<V: View + ?Sized> Render for Document<'_, V> {
    fn render(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        let view = self.0;
        let title = view.title().to_string();
        let frame = view.frame();
        html! {
            doctype
            html lang="en" {
                head {
                    meta charset="utf-8";
                    meta name="viewport" content="width=device-width, initial-scale=1";
                    meta name="color-scheme" content="light";
                    @if view.refresh() { meta http-equiv="refresh" content="3"; }
                    title { (&title) " - uses.ink" }
                    link rel="stylesheet" href=(Url::new(Asset::Tokens.path()));
                    link rel="icon" href=(Url::new(Asset::Mark.path())) type="image/svg+xml";
                    style { (look::SHEET) }
                }
                body {
                    a.look::SKIP href=(Url::new("#main")) { "Skip to the page" }
                    div.look::APP {
                        header.look::RAIL {
                            a.look::BRAND href=(Url::new("/")) {
                                img src=(Url::new(Asset::Mark.path())) alt="" width=34 height=34;
                                "uses.ink"
                            }
                            @if let Some(handle) = &frame.site {
                                p.look::PICK { "your site" b { (Escaped(handle)) } }
                            }
                            (Nav(frame))
                        }
                        main #main.look::PAGE {
                            h1 { (&title) }
                            (Lazy(|out: &mut dyn Sink| view.body(out)))
                        }
                    }
                }
            }
        }
        .render(out)
    }
}

impl<V: View + ?Sized> Document<'_, V> {
    /// The whole page as text.
    pub fn write(&self) -> Result<String, crate::Failure> {
        let mut text = String::new();
        self.render(&mut text).map_err(|halt| crate::Failure::Render { source: crate::Cause::of(&halt) })?;
        Ok(text)
    }
}

/// The navigation: the screens of the site, then Account.
#[derive(Clone, Copy, Debug)]
struct Nav<'a>(&'a Frame);

impl Render for Nav<'_> {
    fn render(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        let frame = self.0;
        let mut screens: Vec<(Screen, Path)> = Vec::new();
        if let Some(handle) = &frame.site {
            screens.extend(Screen::OF_SITE.into_iter().map(|screen| (screen, Path::of(screen, handle))));
        }
        screens.push((Screen::Account, Path::account()));
        html! {
            nav aria-label="Screens" {
                ul.look::NAV {
                    @for (screen, path) in &screens {
                        @if *screen == Screen::Account {
                            li.look::ME {
                                a.look::TAB href=(path) aria-current=(Self::current(frame, *screen)) {
                                    img src=(Url::new(Asset::Face.path())) alt="" width=38 height=38;
                                    (screen.name())
                                }
                            }
                        } @else {
                            li {
                                a.look::TAB href=(path) aria-current=(Self::current(frame, *screen)) { (screen.name()) }
                            }
                        }
                    }
                }
            }
        }
        .render(out)
    }
}

impl Nav<'_> {
    fn current(frame: &Frame, screen: Screen) -> Option<unspliced::keyword::AriaCurrent> {
        (frame.here == Some(screen)).then_some(unspliced::keyword::AriaCurrent::Page)
    }
}

/// A word of a page's text with its glyph, so state never rides on colour alone.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Glyph(pub(crate) &'static str, pub(crate) &'static str);

impl Render for Glyph {
    fn render(&self, out: &mut dyn Sink) -> Result<(), Halt> {
        html! { span.look::GLYPH aria-hidden="true" { (self.0) } (Escaped(self.1)) }.render(out)
    }
}
