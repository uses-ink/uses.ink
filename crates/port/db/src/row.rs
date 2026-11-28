//! Rows, read back as typed columns.

use std::fmt;

use sea_query::Iden;

use crate::{Kind, Mismatch, Value};

/// A type a column can be read as. It is a trait of its own, not `TryFrom<Value>`, because
/// `Option<T>` reading null as none would overlap std's `From<T> for Option<T>`.
pub trait Column: Sized {
    /// Reads `value` as `Self`, or says what it was instead.
    fn column(value: Value) -> Result<Self, Mismatch>;
}

impl Column for Value {
    fn column(value: Value) -> Result<Self, Mismatch> {
        Ok(value)
    }
}

impl Column for i64 {
    fn column(value: Value) -> Result<Self, Mismatch> {
        match value {
            Value::Integer(n) => Ok(n),
            other => Err(Mismatch::of(Kind::Integer, &other)),
        }
    }
}

impl Column for u64 {
    /// A negative integer is a mismatch: counts and sizes are never negative.
    fn column(value: Value) -> Result<Self, Mismatch> {
        match value {
            Value::Integer(n) => Self::try_from(n).map_err(|_| Mismatch::Range { found: n }),
            other => Err(Mismatch::of(Kind::Integer, &other)),
        }
    }
}

impl Column for u32 {
    fn column(value: Value) -> Result<Self, Mismatch> {
        let wide = u64::column(value)?;
        Self::try_from(wide).map_err(|_| Mismatch::Range { found: i64::try_from(wide).unwrap_or(i64::MAX) })
    }
}

impl Column for bool {
    /// Zero is false and one is true; any other integer is a mismatch.
    fn column(value: Value) -> Result<Self, Mismatch> {
        match value {
            Value::Integer(0) => Ok(false),
            Value::Integer(1) => Ok(true),
            Value::Integer(n) => Err(Mismatch::Range { found: n }),
            other => Err(Mismatch::of(Kind::Integer, &other)),
        }
    }
}

impl Column for f64 {
    fn column(value: Value) -> Result<Self, Mismatch> {
        match value {
            Value::Real(n) => Ok(n),
            // D1 hands numbers over as JavaScript numbers, so it cannot tell 2.0 from 2
            #[expect(clippy::cast_precision_loss, reason = "an integer within 2^53 is exact as a float")]
            Value::Integer(n) if n.unsigned_abs() <= Value::SAFE.unsigned_abs() => Ok(n as f64),
            other => Err(Mismatch::of(Kind::Real, &other)),
        }
    }
}

impl Column for String {
    fn column(value: Value) -> Result<Self, Mismatch> {
        match value {
            Value::Text(text) => Ok(text),
            other => Err(Mismatch::of(Kind::Text, &other)),
        }
    }
}

impl Column for Vec<u8> {
    fn column(value: Value) -> Result<Self, Mismatch> {
        match value {
            Value::Blob(bytes) => Ok(bytes),
            other => Err(Mismatch::of(Kind::Blob, &other)),
        }
    }
}

impl<T: Column> Column for Option<T> {
    /// Null is none.
    fn column(value: Value) -> Result<Self, Mismatch> {
        match value {
            Value::Null => Ok(None),
            other => T::column(other).map(Some),
        }
    }
}

/// One row a statement returned: its columns, by name, in the order the statement named them.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Row {
    columns: Vec<(String, Value)>,
}

impl Row {
    /// A row of these columns.
    pub fn new(columns: Vec<(String, Value)>) -> Self {
        Self { columns }
    }

    /// The named column as a `T`.
    #[allow(clippy::needless_pass_by_value, reason = "a column is a Copy identifier, named by value at the call")]
    pub fn get<T: Column>(&self, column: impl Iden) -> Result<T, ColumnError> {
        let name = column.unquoted();
        let value = self
            .columns
            .iter()
            .find(|(found, _)| found == name)
            .map(|(_, value)| value.clone())
            .ok_or_else(|| ColumnError { column: name.to_owned(), fault: ColumnFault::Missing })?;
        T::column(value).map_err(|mismatch| ColumnError { column: name.to_owned(), fault: ColumnFault::Type(mismatch) })
    }

    /// The columns' names and values, in order.
    pub fn columns(&self) -> impl Iterator<Item = (&str, &Value)> {
        self.columns.iter().map(|(name, value)| (name.as_str(), value))
    }
}

/// A column that could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ColumnError {
    /// The column asked for.
    pub column: String,
    /// What was wrong with it.
    pub fault: ColumnFault,
}

/// What was wrong with a column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ColumnFault {
    /// The row has no such column.
    Missing,
    /// The value is of another kind.
    Type(Mismatch),
}

impl fmt::Display for ColumnError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.fault {
            ColumnFault::Missing => write!(out, "the row has no column `{}`", self.column),
            ColumnFault::Type(mismatch) => write!(out, "column `{}`: {mismatch}", self.column),
        }
    }
}

impl std::error::Error for ColumnError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.fault {
            ColumnFault::Missing => None,
            ColumnFault::Type(mismatch) => Some(mismatch),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[derive(sea_query::Iden, Clone, Copy)]
    enum Probe {
        N,
        T,
        Z,
        Gone,
    }

    fn row() -> Row {
        Row::new(vec![
            ("n".into(), Value::Integer(7)),
            ("t".into(), Value::Text("x".into())),
            ("z".into(), Value::Null),
        ])
    }

    #[test]
    fn reads_typed_columns() {
        let row = row();
        assert_eq!(row.get::<i64>(Probe::N).unwrap(), 7);
        assert_eq!(row.get::<String>(Probe::T).unwrap(), "x");
        assert_eq!(row.get::<Option<String>>(Probe::Z).unwrap(), None);
    }

    #[test]
    fn names_the_column_it_could_not_read() {
        let row = row();
        let wrong = row.get::<String>(Probe::N).unwrap_err();
        assert_eq!(wrong.column, "n");
        assert!(matches!(wrong.fault, ColumnFault::Type(Mismatch::Kind { wanted: Kind::Text, found: Kind::Integer })));
        assert_eq!(row.get::<i64>(Probe::Gone).unwrap_err().fault, ColumnFault::Missing);
    }

    #[test]
    fn a_negative_count_is_refused() {
        assert!(u64::column(Value::Integer(-1)).is_err());
        assert!(bool::column(Value::Integer(2)).is_err());
    }

    #[test]
    fn a_blob_shows_only_its_length() {
        assert_eq!(format!("{:?}", Value::Blob(vec![1, 2, 3])), "Blob(3 bytes)");
    }
}
