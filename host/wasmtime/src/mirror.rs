//! The model and its WIT mirror, converted both ways: the parts of `wit/mirror/`, written once
//! for both sides of the boundary, over this crate's bindings. Each part names the generated
//! types through `super::wit`.

use crate::bind::plugin::ink as wit;

mod refusal {
    include!("../../../wit/mirror/refusal.rs");
}
mod written {
    include!("../../../wit/mirror/written.rs");
}
mod said {
    include!("../../../wit/mirror/said.rs");
}
mod document {
    include!("../../../wit/mirror/document.rs");
}
mod site {
    include!("../../../wit/mirror/site.rs");
}
mod host {
    include!("../../../wit/mirror/host.rs");
}
mod snapshot {
    include!("../../../wit/mirror/snapshot.rs");
}

pub use refusal::{Malformed, Unmirrored};
pub(crate) use snapshot::Seen;
