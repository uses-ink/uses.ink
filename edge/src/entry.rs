//! Serving what the deploy holds: a route's entry, its 404 page, a moved route.

use std::fmt;

use model::{HeaderName, Route, Served};

use crate::answer::{Fault, Outcome, Ranging};
use crate::edge::{Call, Edge, Made, Reply};
use crate::fields::{Fields, Hardening};
use crate::port::{Blob, Blobs, Sites};
use crate::range::{Ignored, Pick, Span};
use crate::refusal::Refusal;
use crate::request::Method;
use crate::response::{Body, Name, Status};
use crate::tag::Tag;

/// What an entry is served as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Found {
    /// The route the request names.
    Route,
    /// The site's not-found page, for a route the deploy does not hold.
    Page,
}

/// A route as a `Location`: ASCII only, every other byte escaped.
#[derive(Clone, Copy, Debug)]
struct Location<'a>(&'a Route);

impl fmt::Display for Location<'_> {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0.as_str().bytes() {
            if byte.is_ascii_alphanumeric() || b"-._~/".contains(&byte) {
                write!(out, "{}", char::from(byte))?;
            } else {
                write!(out, "%{byte:02X}")?;
            }
        }
        Ok(())
    }
}

impl<S: Sites, B: Blobs<Scope = S::Scope>> Edge<S, B> {
    /// The entry of the route, its move, or the not-found page.
    pub(crate) async fn found(&self, call: &Call<'_, S::Scope>) -> Reply<S, B> {
        let deploy = &call.resolved.deploy;
        let Some(route) = &call.route else { return self.missing(call).await };
        if let Some(entry) = deploy.manifest.entry(route) {
            return self.entry(call, entry, Found::Route).await;
        }
        match deploy.moved.to(route) {
            Some(to) => self.moved(call, route, to),
            None => self.missing(call).await,
        }
    }

    /// A `301` to the route that took the old one's place. The target is a validated route, never
    /// text from the request.
    fn moved(&self, call: &Call<'_, S::Scope>, from: &Route, to: &Route) -> Reply<S, B> {
        let mut fields = Fields::default();
        fields.set(Name::Location, Location(to));
        fields.set(Name::ContentLength, 0);
        fields.set(Name::Entry(HeaderName::CacheControl), "public, max-age=3600");
        let outcome = Outcome::Moved { from: from.clone(), to: to.clone() };
        self.finish(&call.caller, Made::new(Status::Moved, fields, Body::Empty, outcome))
    }

    /// `404`: the site's page when the manifest has one, else a line of text.
    async fn missing(&self, call: &Call<'_, S::Scope>) -> Reply<S, B> {
        if let Some(entry) = call.resolved.deploy.manifest.entry(&self.page) {
            return self.entry(call, entry, Found::Page).await;
        }
        let text = "not found";
        let mut fields = Fields::default();
        fields.set(Name::ContentType, model::MediaType::Text);
        fields.set(Name::ContentLength, text.len());
        let body = Body::Text(text.to_owned());
        self.finish(
            &call.caller,
            Made::new(Status::NotFound, fields, body, Outcome::Missing { route: call.route.clone() }),
        )
    }

    /// One entry: conditional, ranged, compressed or whole.
    async fn entry(&self, call: &Call<'_, S::Scope>, entry: &Served, found: Found) -> Reply<S, B> {
        let (asked, deploy) = (call.asked, &call.resolved.deploy);
        let status = if found == Found::Page { Status::NotFound } else { Status::from(entry.status()) };
        let cacheable = status == Status::Ok;
        let conditions = &asked.conditions;
        let ranged = cacheable && asked.method == Method::Get && conditions.range.is_some();
        let variant = if cacheable && !ranged {
            conditions
                .accept
                .choose(deploy.variants.held(entry.blob()))
                .and_then(|encoding| deploy.variants.of(entry.blob(), encoding).map(|hash| (encoding, hash)))
        } else {
            None
        };
        let encoding = variant.map(|(encoding, _)| encoding);
        let tag = Tag::of(entry.blob(), encoding);
        let mut fields = Fields::default();
        if deploy.variants.held(entry.blob()).next().is_some() {
            fields.set(Name::Vary, "accept-encoding");
        }
        let route = call.route.clone().unwrap_or_else(Route::root);
        if cacheable && conditions.none_match.as_ref().is_some_and(|matching| matching.holds(&tag)) {
            fields.set(Name::ETag, &tag);
            fields.entry(entry.headers());
            fields.set(Name::Entry(HeaderName::CacheControl), Hardening::REVALIDATE);
            let made = Made::new(Status::NotModified, fields, Body::Empty, Outcome::Unchanged { route });
            return self.finish(&call.caller, made);
        }
        let hash = variant.map_or(entry.blob(), |(_, hash)| hash);
        let blob = match self.blobs.open(&call.resolved.scope, hash).await {
            Ok(blob) => blob,
            Err(source) => return self.fail(Fault::Blob(source), &call.caller),
        };
        let length = blob.length();
        let limit = self.limits.file.count();
        if length > limit {
            return self.fail(Fault::Large { hash, length, limit }, &call.caller);
        }
        let (status, ranging, span) = match conditions.range.as_ref().filter(|_| ranged) {
            None => (status, Ranging::Whole, Span::whole(length)),
            Some(ranges) => {
                let stale = conditions.if_range.as_ref().is_some_and(|validator| !validator.holds(&tag));
                match if stale { Pick::Whole(Ignored::Validator) } else { ranges.pick(length) } {
                    Pick::Span(span) => (Status::Partial, Ranging::Span(span), span),
                    Pick::Whole(why) => (status, Ranging::Ignored(why), Span::whole(length)),
                    Pick::Unsatisfiable => return self.refuse(Refusal::Unsatisfiable { length }, &call.caller),
                }
            }
        };
        fields.set(Name::ContentType, entry.media());
        fields.set(Name::ContentLength, span.length);
        if cacheable {
            fields.set(Name::ETag, &tag);
        }
        match (encoding, cacheable) {
            (Some(encoding), _) => fields.set(Name::ContentEncoding, encoding),
            (None, true) => fields.set(Name::AcceptRanges, "bytes"),
            (None, false) => {}
        }
        if let Ranging::Span(span) = ranging {
            let last = span.last().unwrap_or(span.start);
            fields.set(Name::ContentRange, format_args!("bytes {}-{last}/{length}", span.start));
        }
        fields.entry(entry.headers());
        fields.set(Name::Entry(HeaderName::CacheControl), Hardening::REVALIDATE);
        let body = if asked.method == Method::Head { Body::Empty } else { Body::Blob { blob, span } };
        let outcome = match found {
            Found::Route => Outcome::Served { route, ranging, encoding },
            Found::Page => Outcome::Missing { route: call.route.clone() },
        };
        self.finish(&call.caller, Made::new(status, fields, body, outcome))
    }
}
