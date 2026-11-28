//! Each decision's precedence, one line per code: the witnesses in order, first first. This is
//! the precedence table of `research/architecture.md` 3.6 as types; reading a line is reading
//! the rule.

use model::code;

use crate::interpret::decide::ladder::Ladder;
use crate::interpret::decide::witness::{
    Body, Changed, Content, Created, Dated, Derived, Docs, Draft, Drafts, Exclude, Fallback, FileName, Folder, Front,
    Heading, Hidden, History, Implied, Include, Inline, Obsidian, Otherwise, Pattern, Pin, Posts, Repository, Stem,
    Templates,
};
use crate::interpret::pipeline::file::asset::Used;
use crate::interpret::pipeline::site::Index;

/// `site.format`: a pin, then a vault's `.obsidian/` at the top, then the markdown format.
pub(crate) type Format = Ladder<code::Format, (Pin<code::Format>, Folder<Obsidian>, Fallback<code::Format>)>;
/// `site.root`: the one content folder, then the top.
pub(crate) type Root = Ladder<code::Root, (Pin<code::Root>, Folder<Content>, Fallback<code::Root>)>;
/// `site.layout`: a vault, docs, a blog by its folder or a dated name, then flat.
pub(crate) type Layout = Ladder<
    code::Layout,
    (Pin<code::Layout>, Folder<Obsidian>, Folder<Docs>, Folder<Posts>, FileName<Dated>, Fallback<code::Layout>),
>;
/// `file.ignore`: `exclude`, then `include`, then hidden and repository files and a vault's
/// templates, then kept.
pub(crate) type Ignored = Ladder<
    code::Ignored,
    (
        Pin<code::Ignored>,
        Pattern<Exclude>,
        Pattern<Include>,
        Folder<Hidden>,
        FileName<Repository>,
        Folder<Templates>,
        Fallback<code::Ignored>,
    ),
>;
/// `page.publish` of a page: `publish`, then `draft`, then `include`, then a draft's name.
pub(crate) type Publish = Ladder<
    code::Publish,
    (Pin<code::Publish>, Front<code::Publish>, Draft, Pattern<Include>, Folder<Drafts>, Fallback<code::Publish>),
>;
/// `page.publish` of a plain file: used by a published page.
pub(crate) type Plain = Ladder<code::Publish, (Pin<code::Publish>, Used, Fallback<code::Publish>)>;
/// `page.kind`: `type`, then a posts folder, then a frontmatter date, then a dated name.
pub(crate) type Kind = Ladder<
    code::Kind,
    (
        Pin<code::Kind>,
        Front<code::Kind>,
        Folder<Posts>,
        Implied<code::Date, Front<code::Date>>,
        Implied<code::Date, FileName<Dated>>,
        Fallback<code::Kind>,
    ),
>;
/// `page.title`: `title`, then the first heading, then the file name.
pub(crate) type Title = Ladder<code::Title, (Pin<code::Title>, Front<code::Title>, Heading, Otherwise<FileName<Stem>>)>;
/// `page.summary`: `description` or `summary`, then the first paragraph.
pub(crate) type Summary =
    Ladder<code::Summary, (Pin<code::Summary>, Front<code::Summary>, Body, Fallback<code::Summary>)>;
/// `page.date`: `date`, then a dated name, then the history's creation.
pub(crate) type Date =
    Ladder<code::Date, (Pin<code::Date>, Front<code::Date>, FileName<Dated>, History<Created>, Fallback<code::Date>)>;
/// `page.updated`: `updated`, then the history's last change.
pub(crate) type Updated =
    Ladder<code::Updated, (Pin<code::Updated>, Front<code::Updated>, History<Changed>, Fallback<code::Updated>)>;
/// `page.slug`: `slug`, then the name after its date, then the page id.
pub(crate) type Slug = Ladder<code::Slug, (Pin<code::Slug>, Front<code::Slug>, FileName<Stem>, Fallback<code::Slug>)>;
/// `page.tags`: `tags` joined with the tags written in the text, then none. The text's rung
/// stands first because it testifies only when it adds a tag; see [`Inline`].
pub(crate) type Tags =
    Ladder<code::Tags, (Pin<code::Tags>, Inline<code::Tags>, Front<code::Tags>, Fallback<code::Tags>)>;
/// `site.home`: an index at the top of the root, then a generated listing.
pub(crate) type Home = Ladder<code::Home, (Pin<code::Home>, Index, Fallback<code::Home>)>;
/// `site.title`: the home's title, then the root folder's name, then untitled.
pub(crate) type SiteTitle =
    Ladder<code::SiteTitle, (Pin<code::SiteTitle>, Derived<code::Title>, Folder<Content>, Fallback<code::SiteTitle>)>;
/// `site.description`: the home's summary, then none.
pub(crate) type Description =
    Ladder<code::Description, (Pin<code::Description>, Derived<code::Summary>, Fallback<code::Description>)>;
/// `site.authors`: no one.
pub(crate) type Authors = Ladder<code::Authors, (Pin<code::Authors>, Fallback<code::Authors>)>;
/// `site.language`: the home's `lang`, then English.
pub(crate) type Language =
    Ladder<code::Language, (Pin<code::Language>, Front<code::Language>, Fallback<code::Language>)>;
/// `site.nav`: the top pages that are neither posts nor the home, by title.
pub(crate) type Nav = Ladder<code::Nav, (Pin<code::Nav>, Fallback<code::Nav>)>;
