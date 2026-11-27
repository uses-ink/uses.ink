// The parts of the mirror, in the order they depend on each other. Included once by each crate
// that converts the model and its generated bindings: `plugin/sdk` and `host/wasmtime`. Each part
// names the generated types through `super::wit`, which the including module defines, so this
// file holds no `use`. The paths below resolve against this file, not the includer.

mod refusal {
    include!("refusal.rs");
}
mod written {
    include!("written.rs");
}
mod said {
    include!("said.rs");
}
mod document {
    include!("document.rs");
}
mod site {
    include!("site.rs");
}
mod host {
    include!("host.rs");
}
mod snapshot {
    include!("snapshot.rs");
}
