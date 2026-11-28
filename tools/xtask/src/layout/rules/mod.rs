//! The rules, one file for each kind of fact they read.

mod alias;
mod code;
mod files;
mod layer;
mod licence;
mod names;
mod order;
mod sql;

pub(crate) use alias::Alias;
pub(crate) use code::{Free, Words};
pub(crate) use files::{Banned, Fanout, Unsafe};
pub(crate) use layer::Layer;
pub(crate) use licence::Licence;
pub(crate) use names::Names;
pub(crate) use order::Order;
pub(crate) use sql::{Found, Sql};
