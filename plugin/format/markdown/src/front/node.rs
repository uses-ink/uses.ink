//! YAML as serde-saphyr gives it: each value with its location, loose enough that one bad value
//! never fails the whole block. [`Yaml`](super::Yaml) turns a node into the model's `Raw`.

use std::fmt;

use serde::Deserialize;
use serde::de::{self, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde_saphyr::Spanned as Located;

/// One YAML value. A number or a boolean is read back from the text as written when a key
/// wants text, so only its kind is kept here.
#[derive(Debug)]
pub(super) enum Node {
    Null,
    Bool(bool),
    Number,
    Text(String),
    List(Vec<Located<Self>>),
    /// A mapping: no value the engine reads is one, so only its place is kept.
    Map,
}

/// The keys of a frontmatter mapping, in the order written, each with its value. A key that is
/// not text is no key the engine reads and is left out.
#[derive(Debug, Default)]
pub(super) struct Entries(pub(super) Vec<(String, Located<Node>)>);

impl<'de> Deserialize<'de> for Node {
    fn deserialize<D: de::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(NodeVisitor)
    }
}

impl<'de> Deserialize<'de> for Entries {
    fn deserialize<D: de::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_map(EntriesVisitor)
    }
}

/// Reads any YAML value into a [`Node`].
struct NodeVisitor;

impl<'de> Visitor<'de> for NodeVisitor {
    type Value = Node;

    fn expecting(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("any YAML value")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Node, E> {
        Ok(Node::Bool(value))
    }

    fn visit_i64<E>(self, _: i64) -> Result<Node, E> {
        Ok(Node::Number)
    }

    fn visit_u64<E>(self, _: u64) -> Result<Node, E> {
        Ok(Node::Number)
    }

    fn visit_i128<E>(self, _: i128) -> Result<Node, E> {
        Ok(Node::Number)
    }

    fn visit_u128<E>(self, _: u128) -> Result<Node, E> {
        Ok(Node::Number)
    }

    fn visit_f64<E>(self, _: f64) -> Result<Node, E> {
        Ok(Node::Number)
    }

    fn visit_str<E>(self, value: &str) -> Result<Node, E> {
        Ok(Node::Text(value.to_owned()))
    }

    fn visit_string<E>(self, value: String) -> Result<Node, E> {
        Ok(Node::Text(value))
    }

    fn visit_unit<E>(self) -> Result<Node, E> {
        Ok(Node::Null)
    }

    fn visit_none<E>(self) -> Result<Node, E> {
        Ok(Node::Null)
    }

    fn visit_some<D: de::Deserializer<'de>>(self, deserializer: D) -> Result<Node, D::Error> {
        Node::deserialize(deserializer)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Node, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = seq.next_element::<Located<Node>>()? {
            items.push(item);
        }
        Ok(Node::List(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Node, A::Error> {
        while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
        Ok(Node::Map)
    }
}

/// Reads a mapping into [`Entries`].
struct EntriesVisitor;

impl<'de> Visitor<'de> for EntriesVisitor {
    type Value = Entries;

    fn expecting(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("a mapping of keys")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Entries, A::Error> {
        let mut entries = Vec::new();
        while let Some((key, value)) = map.next_entry::<Node, Located<Node>>()? {
            if let Node::Text(key) = key {
                entries.push((key, value));
            }
        }
        Ok(Entries(entries))
    }
}
