//! The feeds, Atom and RSS, of a site's dated posts, newest first. An undated post is left out
//! and said so; a site with no dated post has no feeds, and says so.

use jiff::Timestamp;
use jiff::civil::Time;
use jiff::fmt::rfc2822::DateTimePrinter;
use jiff::tz::Offset;
use model::{Announcement, Notice, Page, PageKind, Site, Subject, When};

use crate::error::Broken;
use crate::render::RenderSettings;
use crate::render::xml::{Channel, Item};

/// The channel the feeds write, or `None` when no post is dated.
pub(crate) fn channel<'a>(
    site: &'a Site,
    settings: &'a RenderSettings,
    notices: &mut Vec<Notice>,
) -> Result<Option<Channel<'a>>, Broken> {
    let mut items = Vec::new();
    for page in site.pages().iter().filter(|page| page.kind == PageKind::Post) {
        match page.date {
            Some(date) => items.push(item(page, date, settings)?),
            None => notices.push(Notice::new(Announcement::FeedPostUndated).on(Subject::File(page.file.clone()))),
        }
    }
    items.sort_by(|a, b| b.published.cmp(&a.published).then_with(|| a.url.cmp(&b.url)));
    let Some(updated) = items.iter().map(|item| item.updated).max() else {
        notices.push(Notice::new(Announcement::FeedPostNone).on(Subject::Site));
        return Ok(None);
    };
    let identity = site.identity();
    Ok(Some(Channel {
        title: &identity.title,
        description: identity.description.as_deref(),
        authors: &identity.authors,
        language: identity.language.as_ref(),
        home: settings.base.at(&model::Route::root()),
        updated,
        items,
    }))
}

fn item<'a>(page: &'a Page, date: When, settings: &RenderSettings) -> Result<Item<'a>, Broken> {
    let published = instant(date)?;
    let updated = page.updated.map(instant).transpose()?.map_or(published, |updated| updated.max(published));
    let published_rss = DateTimePrinter::new()
        .timestamp_to_rfc9110_string(&published)
        .map_err(|source| Broken::Date { when: date, source })?;
    Ok(Item {
        title: &page.title,
        summary: page.summary.as_deref(),
        url: settings.base.at(&page.route),
        published,
        updated,
        published_rss,
        tags: &page.tags,
    })
}

/// A feed needs an instant: a bare day is its first instant in UTC, as both formats require a
/// time the author never wrote.
fn instant(when: When) -> Result<Timestamp, Broken> {
    if let When::Instant(at) = when {
        return Ok(at);
    }
    Offset::UTC.to_timestamp(when.day().to_datetime(Time::midnight())).map_err(|source| Broken::Date { when, source })
}
