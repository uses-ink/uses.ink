//! The control plane's migrations.

use db::Step;

/// The control plane's own migration steps, from `migration/`. A host that also runs the local
/// queue applies them together with `queue_local::Local::STEPS` as one [`Steps`].
pub const MIGRATIONS: [Step; 1] = [Step::new("0001_domain.sql", include_str!("../../migration/0001_domain.sql"))];
