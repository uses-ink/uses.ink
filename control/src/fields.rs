//! Reading a stored row back into domain values.

use std::str::FromStr;

use db::Row;
use jiff::Timestamp;
use model::{DeployId, Hash};
use sea_query::IdenStatic;
use serde::Deserialize;
use serde::de::IntoDeserializer;

use crate::id::Id;
use crate::{Handle, Unreadable};

/// The columns of one row, read as the domain values they hold.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Fields<'a>(pub(crate) &'a Row);

impl Fields<'_> {
    pub(crate) fn text(self, column: impl IdenStatic) -> Result<String, Unreadable> {
        Ok(self.0.get(column)?)
    }

    pub(crate) fn count(self, column: impl IdenStatic) -> Result<u64, Unreadable> {
        Ok(self.0.get(column)?)
    }

    pub(crate) fn flag(self, column: impl IdenStatic) -> Result<bool, Unreadable> {
        Ok(self.0.get(column)?)
    }

    pub(crate) fn time(self, column: impl IdenStatic) -> Result<Timestamp, Unreadable> {
        let millis: i64 = self.0.get(column)?;
        Timestamp::from_millisecond(millis).map_err(|source| Unreadable::Time { column: column.as_str(), source })
    }

    pub(crate) fn when(self, column: impl IdenStatic) -> Result<Option<Timestamp>, Unreadable> {
        let millis: Option<i64> = self.0.get(column)?;
        millis
            .map(|n| {
                Timestamp::from_millisecond(n).map_err(|source| Unreadable::Time { column: column.as_str(), source })
            })
            .transpose()
    }

    pub(crate) fn id<K>(self, column: impl IdenStatic) -> Result<Id<K>, Unreadable> {
        self.text(column)?.parse().map_err(|source| Unreadable::Id { column: column.as_str(), source })
    }

    pub(crate) fn handle(self, column: impl IdenStatic) -> Result<Handle, Unreadable> {
        self.text(column)?.parse().map_err(|source| Unreadable::Handle { source })
    }

    pub(crate) fn maybe_hash(self, column: impl IdenStatic) -> Result<Option<Hash>, Unreadable> {
        let text: Option<String> = self.0.get(column)?;
        text.map(|text| Hash::from_str(&text).map_err(|source| Unreadable::Hash { column: column.as_str(), source }))
            .transpose()
    }

    pub(crate) fn deploy(self, column: impl IdenStatic) -> Result<DeployId, Unreadable> {
        let text = self.text(column)?;
        DeployId::deserialize(text.as_str().into_deserializer())
            .map_err(|source: serde::de::value::Error| Unreadable::Deploy { column: column.as_str(), source })
    }

    pub(crate) fn maybe_deploy(self, column: impl IdenStatic) -> Result<Option<DeployId>, Unreadable> {
        let text: Option<String> = self.0.get(column)?;
        text.map(|text| {
            DeployId::deserialize(text.as_str().into_deserializer())
                .map_err(|source: serde::de::value::Error| Unreadable::Deploy { column: column.as_str(), source })
        })
        .transpose()
    }

    /// The column's text as one of the names `names` lists, in the order it lists them.
    pub(crate) fn name<T: Copy>(self, column: impl IdenStatic, names: &[(&str, T)]) -> Result<T, Unreadable> {
        let text = self.text(column)?;
        names
            .iter()
            .find(|(name, _)| *name == text)
            .map(|(_, value)| *value)
            .ok_or(Unreadable::Name { column: column.as_str(), found: text })
    }
}
