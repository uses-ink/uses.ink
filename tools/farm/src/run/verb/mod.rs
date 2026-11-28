//! The verbs: one file each, all of them methods of [`Farm`](crate::run::Farm).

mod bless;
mod cargo;
mod check;
#[cfg(test)]
pub(in crate::run) use check::{Stop, Stopped};
mod status;
mod test;
mod wait;
