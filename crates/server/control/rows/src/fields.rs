//! Reading a stored row back into domain values.

use std::str::FromStr;

use db::Row;
use jiff::Timestamp;
use model::{DeployId, Hash};
use sea_query::IdenStatic;
use serde::Deserialize;
use serde::de::IntoDeserializer;

use crate::id::Id;
use crate::unreadable::Unreadable;

/// The columns of one row, read as the values they hold.
#[derive(Debug, Clone, Copy)]
pub struct Fields<'a>(pub &'a Row);

impl Fields<'_> {
    /// A text column.
    pub fn text(self, column: impl IdenStatic) -> Result<String, Unreadable> {
        Ok(self.0.get(column)?)
    }

    /// A column of a non-negative count.
    pub fn count(self, column: impl IdenStatic) -> Result<u64, Unreadable> {
        Ok(self.0.get(column)?)
    }

    /// A boolean column.
    pub fn flag(self, column: impl IdenStatic) -> Result<bool, Unreadable> {
        Ok(self.0.get(column)?)
    }

    /// A blob column.
    pub fn bytes(self, column: impl IdenStatic) -> Result<Vec<u8>, Unreadable> {
        Ok(self.0.get(column)?)
    }

    /// A column of unix milliseconds, as a time.
    pub fn time(self, column: impl IdenStatic) -> Result<Timestamp, Unreadable> {
        let millis: i64 = self.0.get(column)?;
        Timestamp::from_millisecond(millis).map_err(|source| Unreadable::Time { column: column.as_str(), source })
    }

    /// A nullable column of unix milliseconds, as a time.
    pub fn when(self, column: impl IdenStatic) -> Result<Option<Timestamp>, Unreadable> {
        let millis: Option<i64> = self.0.get(column)?;
        millis
            .map(|n| {
                Timestamp::from_millisecond(n).map_err(|source| Unreadable::Time { column: column.as_str(), source })
            })
            .transpose()
    }

    /// A column of 32 hex digits, as the id of a `K`.
    pub fn id<K>(self, column: impl IdenStatic) -> Result<Id<K>, Unreadable> {
        self.text(column)?.parse().map_err(|source| Unreadable::Id { column: column.as_str(), source })
    }

    /// A column holding an email address.
    pub fn address(self, column: impl IdenStatic) -> Result<mail::Address, Unreadable> {
        self.text(column)?.parse().map_err(|source| Unreadable::Address { column: column.as_str(), source })
    }

    /// A nullable column holding an email address.
    pub fn maybe_address(self, column: impl IdenStatic) -> Result<Option<mail::Address>, Unreadable> {
        let text: Option<String> = self.0.get(column)?;
        text.map(|text| text.parse().map_err(|source| Unreadable::Address { column: column.as_str(), source }))
            .transpose()
    }

    /// A nullable text column.
    pub fn maybe_text(self, column: impl IdenStatic) -> Result<Option<String>, Unreadable> {
        Ok(self.0.get(column)?)
    }

    /// A nullable id column.
    pub fn maybe_id<K>(self, column: impl IdenStatic) -> Result<Option<Id<K>>, Unreadable> {
        let text: Option<String> = self.0.get(column)?;
        text.map(|text| text.parse().map_err(|source| Unreadable::Id { column: column.as_str(), source })).transpose()
    }

    /// A blob column holding a 32-byte digest.
    pub fn digest(self, column: impl IdenStatic) -> Result<crate::digest::Digest, Unreadable> {
        let bytes: Vec<u8> = self.0.get(column)?;
        crate::digest::Digest::try_from(bytes.as_slice())
            .map_err(|_| Unreadable::Digest { column: column.as_str(), length: bytes.len() })
    }

    /// A column holding a key id.
    pub fn key(self, column: impl IdenStatic) -> Result<sig::KeyId, Unreadable> {
        self.text(column)?.parse().map_err(|source| Unreadable::Encoding { column: column.as_str(), source })
    }

    /// A nullable column holding a calendar date.
    pub fn maybe_day(self, column: impl IdenStatic) -> Result<Option<jiff::civil::Date>, Unreadable> {
        let text: Option<String> = self.0.get(column)?;
        text.map(|text| {
            text.parse().map_err(|source| Unreadable::Parsed { column: column.as_str(), source: Box::new(source) })
        })
        .transpose()
    }

    /// A column holding a hash.
    pub fn hash(self, column: impl IdenStatic) -> Result<Hash, Unreadable> {
        let text = self.text(column)?;
        Hash::from_str(&text).map_err(|source| Unreadable::Hash { column: column.as_str(), source })
    }

    /// A nullable column holding a hash.
    pub fn maybe_hash(self, column: impl IdenStatic) -> Result<Option<Hash>, Unreadable> {
        let text: Option<String> = self.0.get(column)?;
        text.map(|text| Hash::from_str(&text).map_err(|source| Unreadable::Hash { column: column.as_str(), source }))
            .transpose()
    }

    /// A column holding a deploy id.
    pub fn deploy(self, column: impl IdenStatic) -> Result<DeployId, Unreadable> {
        let text = self.text(column)?;
        DeployId::deserialize(text.as_str().into_deserializer())
            .map_err(|source: serde::de::value::Error| Unreadable::Deploy { column: column.as_str(), source })
    }

    /// A nullable column holding a deploy id.
    pub fn maybe_deploy(self, column: impl IdenStatic) -> Result<Option<DeployId>, Unreadable> {
        let text: Option<String> = self.0.get(column)?;
        text.map(|text| {
            DeployId::deserialize(text.as_str().into_deserializer())
                .map_err(|source: serde::de::value::Error| Unreadable::Deploy { column: column.as_str(), source })
        })
        .transpose()
    }

    /// The column's text as one of the names `names` lists, in the order it lists them.
    pub fn name<T: Copy>(self, column: impl IdenStatic, names: &[(&str, T)]) -> Result<T, Unreadable> {
        let text = self.text(column)?;
        names
            .iter()
            .find(|(name, _)| *name == text)
            .map(|(_, value)| *value)
            .ok_or(Unreadable::Name { column: column.as_str(), found: text })
    }
}
