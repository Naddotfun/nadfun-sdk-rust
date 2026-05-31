//! All types for the Nad.fun SDK.
//!
//! Organized by version: `v1` (legacy bonding curve + Capricorn CL types)
//! and `v2` (NadFun unified router types).
//!
//! Public re-exports are explicit — the alloy `sol!`-generated contract
//! modules (`IBondingCurve`, `ICapricornCLPool`, `IBondingCurveV2Events`)
//! are kept internal to avoid name collisions and to give us flexibility
//! to swap their backing ABIs without breaking the public surface.

pub mod v1;
pub mod v2;

// ============================================================================
// v1 explicit re-exports
// ============================================================================

// bonding curve events
pub use v1::bonding_curve::{
    decode_bonding_curve_event, BondingCurveEvent, BuyEvent, CreateEvent, EventType, GraduateEvent,
    LockEvent, SellEvent, SyncEvent, CURVE_BUY_SIGNATURE, CURVE_CREATE_SIGNATURE,
    CURVE_GRADUATE_SIGNATURE, CURVE_SELL_SIGNATURE, CURVE_SYNC_SIGNATURE,
    CURVE_TOKEN_LOCKED_SIGNATURE,
};

// trading params + router enum + transaction result + allowance
pub use v1::trade::{
    AllowanceStatus, BuyParams, CurveState, ExactOutBuyParams, ExactOutSellParams,
    ExactOutSellPermitParams, GasPricing, Router, SellParams, SellPermitParams, TokenMetadata,
    TransactionResult,
};

// token creation
pub use v1::create::{
    ActionId, ApiErrorResponse, CreateTokenParams, MetadataInfo, MetadataParams, PostMetadataData,
    PostSaltData, SaltParams, TokenCreationResult, UploadImageData,
};

// creator rewards + API responses
pub use v1::creator::{
    ApiBalanceInfo, ApiCreatorInfo, ApiMarketInfo, ApiTokenInfo, CreatedToken,
    CreatedTokenResponse, CreatorBatchClaimParams, CreatorClaimParams, RewardInfo,
};

// DEX (Capricorn CL) events + pool helpers
pub use v1::dex::{
    decode_burn_event, decode_dex_event, decode_initialize_event, decode_mint_event,
    decode_swap_event, BurnEvent, DexEvent, InitializeEvent, MintEvent, PoolMetadata, SwapEvent,
    BURN_SIGNATURE, INITIALIZE_SIGNATURE, MINT_SIGNATURE, SWAP_SIGNATURE,
};

// Internal-only — needed by `crate::stream::v1::dex` for event decoding but
// not part of the public surface. `pub(crate)` so downstream users don't see
// alloy-generated names collide with v2 equivalents.
pub(crate) use v1::dex::ICapricornCLPool;

// ============================================================================
// v2 explicit re-exports
// ============================================================================

// v2 BondingCurve events
pub use v2::events::{
    decode_v2_bonding_curve_event, V2BondingCurveEvent, V2BuyEvent, V2CreateEvent, V2EventType,
    V2GraduateEvent, V2SellEvent, V2SnipingPenaltyEvent, V2SyncEvent,
};

// v2 trading + creation params
pub use v2::params::{
    V2BuyParams, V2BuyWithNativeParams, V2BuyWithPermitParams, V2CreateParams, V2CreatePayment,
    V2CreateTokenParams, V2CreateWithNativeParams, V2DexType, V2ExactOutBuyParams,
    V2ExactOutBuyWithNativeParams, V2ExactOutSellParams, V2ExactOutSellToNativeParams,
    V2GasEstimationParams, V2PermitParams, V2PrepareCreationParams, V2PreparedCreation,
    V2SellParams, V2SellToNativeParams, V2SellToNativeWithPermitParams, V2SellWithPermitParams,
    V2TokenCreationResult, V2VaultAllocation,
};

// v2 token info (vaults)
pub use v2::token_info::{VaultEntry, VaultState, VaultType};

// v2 on-chain view types (curve state + per-quote protocol config)
pub use v2::curve::V2Curve;
pub use v2::quote_config::V2QuoteConfig;
