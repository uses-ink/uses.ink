// Pages, sites, and what a format read. Part of `wit/mirror`.

use model::{
    Asset, Author, Diagnostic, Dimensions, Excluded, Front, Hash, Identity, Layout, Notice, Page, PageId, PageKind,
    Picture, Read, Root, Site, SiteParts, Span, Spanned, Tag, Variant, When,
};

use super::refusal::{Malformed, Unmirrored};
use super::wit::model::{diagnostic as said, file as served, read as front, site as sited};
use super::written::{Bytes, Written};

impl TryFrom<PageKind> for sited::PageKind {
    type Error = Unmirrored;

    fn try_from(kind: PageKind) -> Result<Self, Unmirrored> {
        Ok(match kind {
            PageKind::Home => Self::Home,
            PageKind::Post => Self::Post,
            PageKind::Page => Self::Page,
            _ => return Err(Unmirrored::Kind),
        })
    }
}

impl From<sited::PageKind> for PageKind {
    fn from(kind: sited::PageKind) -> Self {
        match kind {
            sited::PageKind::Home => Self::Home,
            sited::PageKind::Post => Self::Post,
            sited::PageKind::Page => Self::Page,
        }
    }
}

impl From<&Tag> for sited::Tag {
    fn from(tag: &Tag) -> Self {
        Self { written: tag.written.clone() }
    }
}

impl TryFrom<sited::Tag> for Tag {
    type Error = Malformed;

    fn try_from(tag: sited::Tag) -> Result<Self, Malformed> {
        Written::read(tag.written)
    }
}

impl TryFrom<&Page> for sited::Page {
    type Error = Unmirrored;

    fn try_from(page: &Page) -> Result<Self, Unmirrored> {
        Ok(Self {
            file: page.file.to_string(),
            route: page.route.to_string(),
            kind: page.kind.try_into()?,
            title: page.title.clone(),
            date: page.date.map(|when| when.to_string()),
            updated: page.updated.map(|when| when.to_string()),
            tags: page.tags.iter().map(sited::Tag::from).collect(),
            summary: page.summary.clone(),
            body: (&page.body).try_into()?,
        })
    }
}

impl TryFrom<sited::Page> for Page {
    type Error = Malformed;

    fn try_from(page: sited::Page) -> Result<Self, Malformed> {
        let tags = page.tags.into_iter().map(Tag::try_from).collect::<Result<_, _>>()?;
        let made = Self::new(Written::read(page.file)?, Written::read(page.route)?, page.kind.into(), page.title);
        Ok(made
            .holding(page.body.try_into()?)
            .dated(Written::maybe(page.date)?, Written::maybe(page.updated)?)
            .tagged(tags)
            .summarized(page.summary))
    }
}

impl TryFrom<&Root> for sited::Root {
    type Error = Unmirrored;

    fn try_from(root: &Root) -> Result<Self, Unmirrored> {
        match root {
            Root::Top => Ok(Self::Top),
            Root::Folder(path) => Ok(Self::Folder(path.to_string())),
            _ => Err(Unmirrored::Root),
        }
    }
}

impl TryFrom<Layout> for sited::Layout {
    type Error = Unmirrored;

    fn try_from(layout: Layout) -> Result<Self, Unmirrored> {
        match layout {
            Layout::Blog => Ok(Self::Blog),
            Layout::Vault => Ok(Self::Vault),
            Layout::Docs => Ok(Self::Docs),
            Layout::Flat => Ok(Self::Flat),
            _ => Err(Unmirrored::Layout),
        }
    }
}

impl From<Dimensions> for sited::Dimensions {
    fn from(dimensions: Dimensions) -> Self {
        Self { width: dimensions.width, height: dimensions.height }
    }
}

impl From<sited::Dimensions> for Dimensions {
    fn from(dimensions: sited::Dimensions) -> Self {
        Self::new(dimensions.width, dimensions.height)
    }
}

impl TryFrom<&Picture> for sited::Picture {
    type Error = Unmirrored;

    fn try_from(picture: &Picture) -> Result<Self, Unmirrored> {
        let variants = picture
            .variants
            .iter()
            .map(|variant| {
                Ok(sited::Variant {
                    media: served::Media::try_from(variant.media)?,
                    dimensions: variant.dimensions.into(),
                    route: variant.route.to_string(),
                })
            })
            .collect::<Result<_, Unmirrored>>()?;
        Ok(Self {
            dimensions: picture.dimensions.into(),
            variants,
            placeholder: picture.placeholder.as_ref().map(ToString::to_string),
        })
    }
}

impl TryFrom<sited::Picture> for Picture {
    type Error = Malformed;

    fn try_from(picture: sited::Picture) -> Result<Self, Malformed> {
        let variants = picture
            .variants
            .into_iter()
            .map(|variant| {
                Ok(Variant::new(variant.media.into(), variant.dimensions.into(), Written::read(variant.route)?))
            })
            .collect::<Result<_, Malformed>>()?;
        Ok(Self::new(picture.dimensions.into(), variants, Written::maybe(picture.placeholder)?))
    }
}

impl TryFrom<&Asset> for sited::Asset {
    type Error = Unmirrored;

    fn try_from(asset: &Asset) -> Result<Self, Unmirrored> {
        Ok(Self {
            file: asset.file.to_string(),
            route: asset.route.to_string(),
            blob: asset.blob.as_bytes().to_vec(),
            picture: asset.picture.as_ref().map(sited::Picture::try_from).transpose()?,
        })
    }
}

impl TryFrom<sited::Asset> for Asset {
    type Error = Malformed;

    fn try_from(asset: sited::Asset) -> Result<Self, Malformed> {
        let blob = Hash::try_from(Bytes(asset.blob))?;
        let mut made = Self::new(Written::read(asset.file)?, Written::read(asset.route)?, blob);
        made.picture = asset.picture.map(Picture::try_from).transpose()?;
        Ok(made)
    }
}

impl TryFrom<&Site> for sited::Site {
    type Error = Unmirrored;

    fn try_from(site: &Site) -> Result<Self, Unmirrored> {
        let identity = site.identity();
        let id = |page: &Page| page.id.hash().as_bytes().to_vec();
        Ok(Self {
            identity: sited::Identity {
                title: identity.title.clone(),
                description: identity.description.clone(),
                authors: identity.authors.iter().map(ToString::to_string).collect(),
                language: identity.language.to_string(),
            },
            root: site.root().try_into()?,
            layout: site.layout().try_into()?,
            home: site.home().map(id),
            pages: site.pages().iter().map(sited::Page::try_from).collect::<Result<_, _>>()?,
            assets: site
                .assets()
                .iter()
                .map(sited::Asset::try_from)
                .collect::<Result<_, _>>()?,
            nav: site.nav().iter().map(|page| page.hash().as_bytes().to_vec()).collect(),
            excluded: site
                .excluded()
                .iter()
                .map(|excluded| sited::Excluded {
                    path: excluded.path.to_string(),
                    because: excluded.because.name().to_owned(),
                })
                .collect(),
        })
    }
}

impl TryFrom<sited::Site> for Site {
    type Error = Malformed;

    fn try_from(site: sited::Site) -> Result<Self, Malformed> {
        let wire = site.identity;
        let mut identity = Identity::new(wire.title, Written::read(wire.language)?);
        identity.description = wire.description;
        identity.authors = wire
            .authors
            .into_iter()
            .map(|name| Author::try_from(name).map_err(|source| Malformed::Author { source }))
            .collect::<Result<_, _>>()?;
        let root = match site.root {
            sited::Root::Top => Root::Top,
            sited::Root::Folder(path) => Root::Folder(Written::read(path)?),
        };
        let layout = match site.layout {
            sited::Layout::Blog => Layout::Blog,
            sited::Layout::Vault => Layout::Vault,
            sited::Layout::Docs => Layout::Docs,
            sited::Layout::Flat => Layout::Flat,
        };
        let mut parts = SiteParts::new(identity, root, layout);
        parts.pages = site.pages.into_iter().map(Page::try_from).collect::<Result<_, _>>()?;
        let id = |bytes: Vec<u8>| Hash::try_from(Bytes(bytes)).map(PageId::from);
        parts.home = site.home.map(id).transpose()?;
        parts.nav = site.nav.into_iter().map(id).collect::<Result<_, _>>()?;
        parts.assets = site
            .assets
            .into_iter()
            .map(Asset::try_from)
            .collect::<Result<_, Malformed>>()?;
        parts.excluded = site
            .excluded
            .into_iter()
            .map(|excluded| Ok(Excluded::new(Written::read(excluded.path)?, Written::read(excluded.because)?)))
            .collect::<Result<_, Malformed>>()?;
        Self::new(parts).map_err(|source| Malformed::Site { source })
    }
}

// What a format read.

/// A spanned value through the WIT's `option<tuple<T, span>>`, either way.
struct Spans<T>(T);

impl<T> Spans<Option<&Spanned<T>>> {
    fn mirror<W>(self, value: impl FnOnce(&T) -> Result<W, Unmirrored>) -> Result<Option<(W, said::Span)>, Unmirrored> {
        self.0.map(|spanned| Ok((value(&spanned.value)?, spanned.span.into()))).transpose()
    }
}

impl<W> Spans<Option<(W, said::Span)>> {
    fn read<T>(self, value: impl FnOnce(W) -> Result<T, Malformed>) -> Result<Option<Spanned<T>>, Malformed> {
        self.0.map(|(written, span)| Ok(Spanned::new(value(written)?, span.try_into()?))).transpose()
    }
}

impl TryFrom<&Front> for front::Front {
    type Error = Unmirrored;

    fn try_from(read: &Front) -> Result<Self, Unmirrored> {
        let text = |value: &String| Ok(value.clone());
        let when = |value: &When| Ok(value.to_string());
        let yes = |value: &bool| Ok(*value);
        Ok(Self {
            title: Spans(read.title.as_ref()).mirror(text)?,
            date: Spans(read.date.as_ref()).mirror(when)?,
            updated: Spans(read.updated.as_ref()).mirror(when)?,
            slug: Spans(read.slug.as_ref()).mirror(|slug| Ok(slug.to_string()))?,
            tags: Spans(read.tags.as_ref()).mirror(|tags| Ok(tags.iter().map(sited::Tag::from).collect()))?,
            publish: Spans(read.publish.as_ref()).mirror(yes)?,
            draft: Spans(read.draft.as_ref()).mirror(yes)?,
            kind: Spans(read.kind.as_ref()).mirror(|kind| sited::PageKind::try_from(*kind))?,
            description: Spans(read.description.as_ref()).mirror(text)?,
            language: Spans(read.language.as_ref()).mirror(|language| Ok(language.to_string()))?,
            block: read.block.map(said::Span::from),
        })
    }
}

impl TryFrom<front::Front> for model::Front {
    type Error = Malformed;

    fn try_from(wire: front::Front) -> Result<Self, Malformed> {
        let mut read = Self::default();
        read.title = Spans(wire.title).read(Ok)?;
        read.date = Spans(wire.date).read(Written::read)?;
        read.updated = Spans(wire.updated).read(Written::read)?;
        read.slug = Spans(wire.slug).read(Written::read)?;
        read.tags = Spans(wire.tags).read(|tags| tags.into_iter().map(Tag::try_from).collect())?;
        read.publish = Spans(wire.publish).read(Ok)?;
        read.draft = Spans(wire.draft).read(Ok)?;
        read.kind = Spans(wire.kind).read(|kind| Ok(PageKind::from(kind)))?;
        read.description = Spans(wire.description).read(Ok)?;
        read.language = Spans(wire.language).read(Written::read)?;
        read.block = wire.block.map(Span::try_from).transpose()?;
        Ok(read)
    }
}

impl TryFrom<&Read> for front::Read {
    type Error = Unmirrored;

    fn try_from(read: &Read) -> Result<Self, Unmirrored> {
        Ok(Self {
            document: (&read.document).try_into()?,
            front: (&read.front).try_into()?,
            heading: Spans(read.heading.as_ref()).mirror(|text| Ok(text.clone()))?,
            diagnostics: read.diagnostics.iter().map(said::Diagnostic::try_from).collect::<Result<_, _>>()?,
            notices: read.notices.iter().map(said::Notice::try_from).collect::<Result<_, _>>()?,
        })
    }
}

impl TryFrom<front::Read> for Read {
    type Error = Malformed;

    fn try_from(wire: front::Read) -> Result<Self, Malformed> {
        let heading = Spans(wire.heading).read(Ok)?;
        let mut read = Self::new(wire.document.try_into()?, wire.front.try_into()?, heading);
        read.diagnostics = wire.diagnostics.into_iter().map(Diagnostic::try_from).collect::<Result<_, _>>()?;
        read.notices = wire.notices.into_iter().map(Notice::try_from).collect::<Result<_, _>>()?;
        Ok(read)
    }
}
