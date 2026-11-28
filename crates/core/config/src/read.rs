//! Reading `ink.toml` into a [`Config`]: a walk over toml's spanned tree, so every key and value
//! keeps where it was written. The only module that sees the `toml` crate.

mod node;

use std::str::FromStr;

use model::decision::Scope;
use model::{ContentPath, Diagnostic, Each, Order, PageKind, Problem, Span, Spanned, Subject, Word};
use toml::de::DeTable;

pub(crate) use node::{Key, Node, Refused, Table};

use crate::pin::{KeyPath, Place};
use crate::theme::{Kebab, Theme, Token};
use crate::value::{Cause, Expected, Shape, Toml};
use crate::{Config, Folder, Image, Offsets, Parsed, Patterns, Pinned, Quality, Width, Widths};

/// The walk: what read cleanly so far, and every problem.
#[derive(Debug, Default)]
pub(crate) struct Reader {
    config: Config,
    diagnostics: Vec<Diagnostic>,
}

impl Reader {
    /// Reads all of `text`.
    pub(crate) fn parse(text: &str) -> Parsed {
        let (root, errors) = DeTable::parse_recoverable(text);
        if !errors.is_empty() {
            let diagnostics = errors
                .iter()
                .map(|error| {
                    let diagnostic =
                        Diagnostic::new(Problem::ConfigSyntaxInvalid { reason: error.message().to_owned() });
                    match error.span() {
                        Some(range) => diagnostic.at(Offsets(range).into()),
                        None => diagnostic,
                    }
                })
                .collect();
            return Parsed { config: Config::default(), diagnostics };
        }
        let mut reader = Self::default();
        for (key, node) in Table(root.get_ref()).entries() {
            reader.top(key, node);
        }
        Parsed { config: reader.config, diagnostics: reader.diagnostics }
    }

    fn unknown(&mut self, key: &KeyPath<'_>, at: Span) {
        self.diagnostics.push(Diagnostic::new(Problem::ConfigKeyUnknown { key: key.to_string() }).at(at));
    }

    fn invalid(&mut self, key: &KeyPath<'_>, expected: &Expected<'_>, at: Span) {
        let problem = Problem::ConfigValueInvalid { key: key.to_string(), expected: expected.to_string() };
        self.diagnostics.push(Diagnostic::new(problem).at(at));
    }

    /// The table `node` is, or a diagnostic saying `key` takes one shaped `shape`.
    fn table<'a>(&mut self, key: &KeyPath<'_>, node: Node<'a>, shape: Shape<'_>) -> Option<Table<'a>> {
        let table = node.table();
        if table.is_none() {
            self.invalid(key, &shape.into(), node.span());
        }
        table
    }

    /// The `T` the string `node` spells, or a diagnostic saying `key` takes `shape`, with the
    /// reason `T` gave.
    fn parsed<T>(&mut self, key: &KeyPath<'_>, node: Node<'_>, shape: Shape<'_>) -> Option<Spanned<T>>
    where
        T: FromStr<Err: Into<Cause>>,
    {
        let expected = match node.text().map(str::parse::<T>) {
            Ok(Ok(value)) => return Some(Spanned::new(value, node.span())),
            Ok(Err(error)) => Expected::because(shape, error),
            Err(_) => shape.into(),
        };
        self.invalid(key, &expected, node.span());
        None
    }

    fn top(&mut self, key: Key<'_>, node: Node<'_>) {
        let path = KeyPath(vec![key.name]);
        match key.name {
            "theme" => self.theme(&path, node),
            "image" => self.image(&path, node),
            "include" => self.config.include = self.patterns(&path, node),
            "exclude" => self.config.exclude = self.patterns(&path, node),
            "folder" => self.folders(&path, node),
            table if Place::holds(table, Scope::Site) || Place::holds(table, Scope::File) => {
                let Some(entries) = self.table(&path, node, Shape::Table(table)) else { return };
                for (key, node) in entries.entries() {
                    self.entry(table, key, node);
                }
            }
            _ => self.unknown(&path, key.span),
        }
    }

    /// One entry of a pin table: a sitewide pin, or a file's table of pins. A per-file key
    /// written straight in the table (`[page] title = "x"`) is `config.key.misplaced`, not a file
    /// named `title`.
    fn entry(&mut self, table: &str, key: Key<'_>, node: Node<'_>) {
        let path = KeyPath(vec![table, key.name]);
        if let Some(place) = Place::find(table, key.name, Scope::Site) {
            return place.code.each(&mut Entry { reader: self, path: &path, subject: Subject::Site, key, node });
        }
        if !Place::holds(table, Scope::File) {
            return self.unknown(&path, key.span);
        }
        if node.table().is_none() && Place::find(table, key.name, Scope::File).is_some() {
            let problem = Problem::ConfigKeyMisplaced { key: path.to_string(), table: table.to_owned() };
            return self.diagnostics.push(Diagnostic::new(problem).at(key.span));
        }
        let file = match ContentPath::parse(key.name) {
            Ok(file) => file,
            Err(error) => return self.invalid(&path, &Expected::because(Shape::FileKey, error), key.span),
        };
        let Some(pins) = self.table(&path, node, Shape::Pins { table, file: &file }) else { return };
        for (key, node) in pins.entries() {
            let path = KeyPath(vec![table, file.as_str(), key.name]);
            match Place::find(table, key.name, Scope::File) {
                Some(place) => {
                    let subject = Subject::File(file.clone());
                    place.code.each(&mut Entry { reader: self, path: &path, subject, key, node });
                }
                None => self.unknown(&path, key.span),
            }
        }
    }

    fn patterns(&mut self, path: &KeyPath<'_>, node: Node<'_>) -> Patterns {
        let shape = Shape::Array(&Shape::Pattern);
        let Ok(items) = node.items() else {
            self.invalid(path, &shape.into(), node.span());
            return Patterns::default();
        };
        // every item is checked and said, and a list with a bad item is applied not at all
        let parsed: Vec<_> = items.map(|item| self.parsed(path, item, shape)).collect();
        parsed.into_iter().collect::<Option<Patterns>>().unwrap_or_default()
    }

    /// The `T` the whole number `node` is, or a diagnostic saying `key` takes `shape`, with the
    /// reason `T` gave.
    fn number<T>(&mut self, key: &KeyPath<'_>, node: Node<'_>, shape: Shape<'_>) -> Option<Spanned<T>>
    where
        T: TryFrom<i64, Error: Into<Cause>>,
    {
        let expected = match node.whole().map(T::try_from) {
            Ok(Ok(value)) => return Some(Spanned::new(value, node.span())),
            Ok(Err(error)) => Expected::because(shape, error),
            Err(_) => shape.into(),
        };
        self.invalid(key, &expected, node.span());
        None
    }

    /// The widths `node` lists: every item is checked and said, and a list with a bad item, or
    /// one out of order, is applied not at all.
    fn widths(&mut self, path: &KeyPath<'_>, node: Node<'_>) -> Option<Spanned<Widths>> {
        let Ok(items) = node.items() else {
            self.invalid(path, &Shape::Widths.into(), node.span());
            return None;
        };
        let parsed: Vec<_> = items.map(|item| self.number::<Width>(path, item, Shape::Width)).collect();
        let widths = parsed.into_iter().map(|width| width.map(|width| width.value)).collect::<Option<Vec<_>>>()?;
        match Widths::try_from(widths) {
            Ok(widths) => Some(Spanned::new(widths, node.span())),
            Err(error) => {
                self.invalid(path, &Expected::because(Shape::Widths, error), node.span());
                None
            }
        }
    }

    /// The `[folder]` tables: each key is a folder's path, each value its table.
    fn folders(&mut self, path: &KeyPath<'_>, node: Node<'_>) {
        let Some(tables) = self.table(path, node, Shape::Table("folder")) else { return };
        for (key, node) in tables.entries() {
            let path = path.and(key.name);
            let folder = match ContentPath::parse(key.name) {
                Ok(folder) => folder,
                Err(error) => {
                    self.invalid(&path, &Expected::because(Shape::FileKey, error), key.span);
                    continue;
                }
            };
            let Some(table) = self.table(&path, node, Shape::Folder) else { continue };
            let mut found = Folder::new(folder, key.span);
            for (key, value) in table.entries() {
                let path = path.and(key.name);
                let whole = Offsets(key.span.start()..value.span().end()).into();
                match key.name {
                    "kind" => found.kind = self.kind(&path, value, whole),
                    "weight" => found.weight = self.weight(&path, value, whole),
                    "order" => found.order = self.order(&path, value, whole),
                    _ => self.unknown(&path, key.span),
                }
            }
            self.config.folders.push(found);
        }
    }

    /// `kind` of a folder: a page or a post, not a home.
    fn kind(&mut self, path: &KeyPath<'_>, node: Node<'_>, whole: Span) -> Option<Spanned<PageKind>> {
        match node.text() {
            Ok("page") => Some(Spanned::new(PageKind::Page, whole)),
            Ok("post") => Some(Spanned::new(PageKind::Post, whole)),
            _ => {
                self.invalid(path, &Shape::One(&["page", "post"]).into(), node.span());
                None
            }
        }
    }

    /// `weight` of a folder: a whole number.
    fn weight(&mut self, path: &KeyPath<'_>, node: Node<'_>, whole: Span) -> Option<Spanned<i32>> {
        let weight = node.whole().ok().and_then(|number| i32::try_from(number).ok());
        if weight.is_none() {
            self.invalid(path, &Shape::Weight.into(), node.span());
        }
        weight.map(|weight| Spanned::new(weight, whole))
    }

    /// `order` of a folder: how its pages are listed.
    fn order(&mut self, path: &KeyPath<'_>, node: Node<'_>, whole: Span) -> Option<Spanned<Order>> {
        let order = match node.text() {
            Ok("route") => Order::Route,
            Ok("title") => Order::Title,
            Ok("newest") => Order::Newest,
            Ok("oldest") => Order::Oldest,
            _ => {
                self.invalid(path, &Shape::One(&["route", "title", "newest", "oldest"]).into(), node.span());
                return None;
            }
        };
        Some(Spanned::new(order, whole))
    }

    fn image(&mut self, path: &KeyPath<'_>, node: Node<'_>) {
        let Some(table) = self.table(path, node, Shape::Image) else { return };
        let mut image = Image { span: Some(node.span()), ..Image::default() };
        for (key, value) in table.entries() {
            let path = path.and(key.name);
            match key.name {
                "widths" => image.widths = self.widths(&path, value),
                "avif-quality" => image.avif_quality = self.number::<Quality>(&path, value, Shape::Quality),
                "jpeg-quality" => image.jpeg_quality = self.number::<Quality>(&path, value, Shape::Quality),
                _ => self.unknown(&path, key.span),
            }
        }
        self.config.image = image;
    }

    fn theme(&mut self, path: &KeyPath<'_>, node: Node<'_>) {
        let Some(table) = self.table(path, node, Shape::Theme) else { return };
        let mut theme = Theme { plugin: None, tokens: Vec::new(), span: node.span() };
        for (key, value) in table.entries() {
            let path = path.and(key.name);
            if key.name == "use" {
                theme.plugin = self.parsed(&path, value, Shape::ThemeId);
            } else if Kebab::try_from(key.name).is_err() {
                self.unknown(&path, key.span);
            } else if let Ok(text) = value.text() {
                let name = Spanned::new(key.name.to_owned(), key.span);
                theme.tokens.push(Token { name, value: Spanned::new(text.to_owned(), value.span()) });
            } else {
                self.invalid(&path, &Shape::Text.into(), value.span());
            }
        }
        self.config.theme = Some(theme);
    }
}

/// One pin being read, as the code pinned at its place.
#[derive(Debug)]
struct Entry<'r, 'a> {
    reader: &'r mut Reader,
    /// The pin's whole key, for diagnostics.
    path: &'r KeyPath<'a>,
    subject: Subject,
    key: Key<'a>,
    node: Node<'a>,
}

impl<C: Word<Value: Toml>> Each<C> for Entry<'_, '_> {
    type Out = ();

    fn each(&mut self) {
        match C::Value::read(self.node) {
            Ok(value) => {
                let span = Offsets(self.key.span.start()..self.node.span().end()).into();
                self.reader.config.insert::<C>(self.subject.clone(), Pinned { value, span });
            }
            Err(refused) => {
                for at in refused {
                    self.reader.invalid(self.path, &Shape::pin::<C>().into(), at);
                }
            }
        }
    }
}
