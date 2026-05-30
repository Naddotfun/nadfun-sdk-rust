//! `CoreV1` — v1 namespace handle. Borrows `&Core`; methods delegate to the
//! v1 contract bindings. Construct via [`crate::Core::v1`].

use crate::core::Core;

/// v1 trading/query namespace handle. Zero-cost `Copy` wrapper over `&Core`.
#[derive(Clone, Copy)]
pub struct CoreV1<'a> {
    // Used by methods added in a later task.
    #[allow(dead_code)]
    pub(crate) core: &'a Core,
}

impl<'a> CoreV1<'a> {
    // methods added in a later task
}
