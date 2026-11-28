//! The wiring of a self-host: the product `ink serve --data` runs, its sign-in, its supervised
//! tasks and the settings that govern them.

pub(crate) mod dash;
pub(crate) mod forge;
pub(crate) mod hosting;
pub(crate) mod product;
pub(crate) mod report;
pub(crate) mod signin;
pub(crate) mod tasks;
