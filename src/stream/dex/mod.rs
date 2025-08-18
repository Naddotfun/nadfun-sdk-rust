//! DEX (Uniswap V3) event streaming and indexing
//! 
//! This module provides streaming and indexing functionality specifically for
//! Uniswap V3 swap events across multiple pools.

pub mod indexer;
pub mod stream;

// Re-export main types
pub use indexer::UniswapSwapIndexer;
pub use stream::UniswapSwapStream;