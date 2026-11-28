//! What crosses the JavaScript boundary: values bound to a statement, and cells read from a row.

use std::fmt;

use db::{Unsupported, Value};
use serde::Deserialize;
use serde::de::{self, Deserializer, MapAccess, SeqAccess, Visitor};
use worker::js_sys::Uint8Array;
use worker::wasm_bindgen::JsValue;

use crate::error::Unread;

/// A value bound to a statement, as JavaScript holds it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Bound;

impl Bound {
    /// `value` as D1 binds it: numbers as numbers, bytes as a `Uint8Array`, null as null.
    #[expect(clippy::cast_precision_loss, reason = "an integer within 2^53 is exact as a float")]
    pub(crate) fn js(value: &Value) -> Result<JsValue, Unsupported> {
        Ok(match value {
            Value::Null => JsValue::NULL,
            Value::Integer(n) if n.unsigned_abs() <= Value::SAFE.unsigned_abs() => JsValue::from_f64(*n as f64),
            Value::Integer(n) => return Err(Unsupported::Unsafe { found: *n }),
            Value::Real(n) => JsValue::from_f64(*n),
            Value::Text(text) => JsValue::from_str(text),
            Value::Blob(bytes) => Uint8Array::from(bytes.as_slice()).into(),
            other => return Err(Unsupported::Kind { kind: other.kind() }),
        })
    }
}

/// One cell of a row, as D1 returns it: null, a number, text, or an array of bytes.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Cell {
    Null,
    Number(f64),
    Text(String),
    Bytes(Vec<u8>),
}

impl Cell {
    /// The value this cell holds. A number with no fraction is an integer when it is exact and a
    /// real otherwise: D1 cannot tell `2.0` from `2`. Beyond 2^53 - 1 it cannot tell an integer it
    /// rounded from a real either, so such a number is refused.
    #[expect(clippy::cast_possible_truncation, reason = "checked to be a whole number within 2^53")]
    #[expect(clippy::cast_precision_loss, reason = "2^53 - 1 is exact as a float")]
    pub(crate) fn value(self) -> Result<Value, Unread> {
        Ok(match self {
            Self::Null => Value::Null,
            Self::Number(n) if !n.is_finite() => return Err(Unread::Finite),
            Self::Number(n) if n.fract() == 0.0 && n.abs() <= Value::SAFE as f64 => Value::Integer(n as i64),
            Self::Number(n) if n.fract() == 0.0 => return Err(Unread::Rounded { found: n }),
            Self::Number(n) => Value::Real(n),
            Self::Text(text) => Value::Text(text),
            Self::Bytes(bytes) => Value::Blob(bytes),
        })
    }
}

impl<'de> Deserialize<'de> for Cell {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(Reading)
    }
}

/// Reads one cell from whatever JavaScript value it is.
struct Reading;

impl<'de> Visitor<'de> for Reading {
    type Value = Cell;

    fn expecting(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("null, a number, text or an array of bytes")
    }

    fn visit_unit<E: de::Error>(self) -> Result<Cell, E> {
        Ok(Cell::Null)
    }

    fn visit_none<E: de::Error>(self) -> Result<Cell, E> {
        Ok(Cell::Null)
    }

    #[expect(clippy::cast_precision_loss, reason = "serde_wasm_bindgen gives integers only within 2^53")]
    fn visit_i64<E: de::Error>(self, n: i64) -> Result<Cell, E> {
        Ok(Cell::Number(n as f64))
    }

    #[expect(clippy::cast_precision_loss, reason = "serde_wasm_bindgen gives integers only within 2^53")]
    fn visit_u64<E: de::Error>(self, n: u64) -> Result<Cell, E> {
        Ok(Cell::Number(n as f64))
    }

    fn visit_f64<E: de::Error>(self, n: f64) -> Result<Cell, E> {
        Ok(Cell::Number(n))
    }

    fn visit_str<E: de::Error>(self, text: &str) -> Result<Cell, E> {
        Ok(Cell::Text(text.to_owned()))
    }

    fn visit_string<E: de::Error>(self, text: String) -> Result<Cell, E> {
        Ok(Cell::Text(text))
    }

    fn visit_bytes<E: de::Error>(self, bytes: &[u8]) -> Result<Cell, E> {
        Ok(Cell::Bytes(bytes.to_vec()))
    }

    fn visit_byte_buf<E: de::Error>(self, bytes: Vec<u8>) -> Result<Cell, E> {
        Ok(Cell::Bytes(bytes))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Cell, A::Error> {
        let mut bytes = Vec::with_capacity(seq.size_hint().unwrap_or(0));
        while let Some(byte) = seq.next_element::<u8>()? {
            bytes.push(byte);
        }
        Ok(Cell::Bytes(bytes))
    }
}

/// One row as D1 returns it from `run` and from a batch: the columns as an object, in order.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Record(pub(crate) Vec<(String, Cell)>);

impl<'de> Deserialize<'de> for Record {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_map(Columns)
    }
}

/// Reads the entries of a row's object.
struct Columns;

impl<'de> Visitor<'de> for Columns {
    type Value = Record;

    fn expecting(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str("an object of columns")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Record, A::Error> {
        let mut columns = Vec::with_capacity(map.size_hint().unwrap_or(0));
        while let Some(entry) = map.next_entry::<String, Cell>()? {
            columns.push(entry);
        }
        Ok(Record(columns))
    }
}
