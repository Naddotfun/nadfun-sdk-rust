//! `CoreV2` — v2 namespace handle. Borrows `&Core`; methods delegate to the
//! v2 contract bindings. Construct via [`crate::Core::v2`].

use crate::core::Core;

/// v2 trading/query namespace handle. Zero-cost `Copy` wrapper over `&Core`.
#[derive(Clone, Copy)]
pub struct CoreV2<'a> {
    // Used by methods added in a later task.
    #[allow(dead_code)]
    pub(crate) core: &'a Core,
}

impl<'a> CoreV2<'a> {
    // methods added in a later task
}
