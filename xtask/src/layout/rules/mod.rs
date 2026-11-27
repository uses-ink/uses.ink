//! The rules, one file for each kind of fact they read.

mod code;
mod files;
mod order;
mod sql;

pub(crate) use code::{Free, Words};
pub(crate) use files::{Banned, Unsafe};
pub(crate) use order::{Alias, Licence, Names, Order};
pub(crate) use sql::{Found, Sql};
