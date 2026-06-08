//! DEX (Capricorn CL) event streaming and indexing
//!
//! This module provides streaming and indexing functionality for
//! DEX swap events across multiple pools.

pub mod indexer;
pub mod stream;

// Re-export main types
pub use indexer::DexIndexer;
pub use stream::DexStream;
