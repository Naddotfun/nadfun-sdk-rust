# Token Creation Flow Documentation

This document describes the complete flow for creating a token on Nad.fun, for
**both v1 and v2**. The off-chain pipeline (image → metadata → salt) is shared;
the on-chain step differs by version (v1 `BondingCurveRouter` vs v2
`NadFunRouter`), and so does the initial-buy fee model.

The API surface mirrors the Nad.fun API integration guide
(`github.com/Naddotfun/api-intergration`); the on-chain sections track
`nadfun-contract-v2` (`INadFunRouter` / `NadFunRouter` / `BondingCurve`) for v2
and the v1 `BondingCurveRouter` for v1.

> SDK note: `ApiClient` (`nadfun_sdk::api`) wraps the three off-chain calls, and
> `core.v1().create_token(..)` / `core.v2().create_token(..)` orchestrate the
> full off-chain → on-chain flow. The raw API is documented here for integrators
> who do not use the SDK.

---

## Overview

```
            shared off-chain pipeline                       on-chain create
┌───────────────┐   ┌──────────────────┐   ┌─────────────┐   ┌────────────────────────┐
│  Upload Image │──>│ Upload Metadata  │──>│  Mine Salt  │──>│  v1: BondingCurveRouter│
│/metadata/image│   │/metadata/metadata│   │ /token/salt │   │      .create(..)       │
└───────────────┘   └──────────────────┘   └─────────────┘   │  v2: NadFunRouter      │
       │                    │                      │          │ .create / .createWith  │
       ▼                    ▼                      ▼          │        Native(..)      │
   image_uri          metadata_uri        salt + predicted   └────────────────────────┘
    is_nsfw                                   address                     │
                                                                          ▼
                                                          token live → index via /token/:token
```

| Step | Endpoint / call | Shared? | Output |
|------|-----------------|---------|--------|
| 1. Upload Image | `POST /metadata/image` | shared (v1 + v2) | `image_uri`, `is_nsfw` |
| 2. Upload Metadata | `POST /metadata/metadata` | shared | `metadata_uri` |
| 3. Mine Salt | `POST /token/salt` (`version: "V1"`/`"V2"`) | shared, version-tagged | `salt`, predicted `address` |
| 4. On-chain create | v1 `BondingCurveRouter.create` / v2 `NadFunRouter.create[WithNative]` | **version-specific** | `token`, (v2) `tokenOut` |
| 5. Index | `GET /token/:token`, `/trade/*` | shared | live token + market data |

Salt mining is CREATE2 over an EIP-1167 minimal-proxy clone of the version's
Token implementation, deployed by that version's bonding curve. **Pass the
correct `version`** to `/token/salt` — a v1 salt predicts a different address
than a v2 salt for the same name/symbol/creator, and using the wrong one makes
the on-chain create revert or deploy to an unexpected address.

---

## Step 1: Upload Image

Upload the token image with automatic NSFW validation. Shared by v1 and v2.

### Endpoint
```
POST /metadata/image
```

### Request

**Content-Type**: `image/png`, `image/jpeg`, `image/webp`, or `image/svg+xml`
**Body**: raw binary image bytes
**Size Limit**: 5MB maximum (format detected from magic bytes, not just the header)

```bash
curl -X POST {BASE_URL}/metadata/image \
  -H "Content-Type: image/png" \
  --data-binary @./my-token-image.png
```

### Response — `200 OK`

```json
{
  "image_uri": "https://storage.nadapp.net/coin/94a412d2-b599-4bb0-b026-b14c4036c58c",
  "is_nsfw": false
}
```

- `image_uri` (string): CDN URL of the uploaded image (used in Step 2).
- `is_nsfw` (boolean): NSFW classification result.

| Status | Description |
|--------|-------------|
| 400 | Invalid image format or missing image |
| 413 | Image exceeds 5MB limit |
| 500 | NSFW check or upload failed |

---

## Step 2: Upload Metadata

Store token metadata JSON using the `image_uri` from Step 1. Shared by v1 and v2.
The NSFW result is cached after image upload — call this soon after Step 1 or the
cache expires and metadata upload fails.

### Endpoint
```
POST /metadata/metadata
```

### Request — `application/json`

```json
{
  "image_uri": "https://storage.nadapp.net/coin/94a412d2-...",
  "name": "My Token",
  "symbol": "MTK",
  "description": "An awesome token for the NAD community",
  "website": "https://mytoken.com",
  "twitter": "https://x.com/mytoken",
  "telegram": "https://t.me/mytoken"
}
```

| Field | Required | Rule |
|-------|:--------:|------|
| `image_uri` | Yes | Must start with the allowed image domain (e.g. `https://storage.nadapp.net/`) |
| `name` | Yes | Trimmed length 1–32, no newlines |
| `symbol` | Yes | 1–10 chars, ASCII alphanumeric |
| `description` | No | ≤ 500 chars (integrations should send a non-empty value) |
| `website` | No | If present, must start with `https://` |
| `twitter` | No | If present, must start with `https://x.com/` |
| `telegram` | No | If present, must start with `https://t.me/` |

### Response — `200 OK`

```json
{
  "metadata_uri": "https://storage.nadapp.net/metadata/94a412d2-...json",
  "metadata": {
    "name": "My Token", "symbol": "MTK",
    "description": "An awesome token for the NAD community",
    "image_uri": "https://storage.nadapp.net/coin/94a412d2-...",
    "website": "https://mytoken.com",
    "twitter": "https://x.com/mytoken",
    "telegram": "https://t.me/mytoken",
    "is_nsfw": false
  }
}
```

The salt server normalizes `name`/`symbol`; the on-chain create **must** use the
server-returned values (the CREATE2 hash is computed against them).

---

## Step 3: Mine Salt (CREATE2 vanity address)

Mine a `bytes32` salt so the predicted token clone address ends with the
configured vanity suffix (e.g. `7777`). **Version-tagged** — pass `version`.

### Endpoint
```
POST /token/salt
```

### Request — `application/json`

```json
{
  "creator": "0x742d35Cc6634C0532925a3b844Bc9e7595f70143",
  "name": "My Token",
  "symbol": "MTK",
  "metadata_uri": "https://storage.nadapp.net/metadata/94a412d2-...json",
  "version": "V2"
}
```

| Field | Required | Rule |
|-------|:--------:|------|
| `creator` | Yes | EVM address — **must equal the wallet that signs the create tx** (the curve uses `msg.sender` as creator) |
| `name` / `symbol` | Yes | Must match the metadata (server-normalized) |
| `metadata_uri` | Yes | From Step 2 |
| `version` | No | `"V1"` or `"V2"` (default `"V1"`). Selects the curve + token implementation the CREATE2 address is mined against |

### Response — `200 OK`

```json
{
  "salt": "0x000000000000000000000000000000000000000000000000000000000000a3f5",
  "address": "0x742d35Cc6634C0532925a3b844Bc9e7595f7777"
}
```

The `address` is a **prediction** — verify it against the on-chain `Create`
event / transaction receipt after Step 4. The SDK's `create_token` does this
automatically and errors on any predicted-vs-on-chain mismatch.

| Status | Description |
|--------|-------------|
| 400 | Invalid parameters (e.g. bad creator address) |
| 500 | Max iterations reached / internal error |

---

## Step 4: On-chain create — version-specific

### v1 — `BondingCurveRouter.create`

v1 has a single native (MON) quote and one shared genesis curve.

- The client computes the initial-buy `amountOut` **up front** and passes it in.
  Use the on-chain Lens helper, exposed by the SDK as
  `core.v1().get_initial_buy_amount_out(amount_in)` — **parameterless** beyond
  the MON amount, because all v1 tokens share one genesis curve. It is an
  on-chain Lens passthrough, so it is exact by construction.
- `msg.value = deploy_fee + initial_buy` (the SDK adds the deploy fee on top of
  `value`).
- There is **no per-token creator fee** on the v1 curve.

```rust
let initial_buy = parse_ether("1")?;            // MON
let amount_out = core.v1().get_initial_buy_amount_out(initial_buy).await?;
let result = core.v1().create_token(
    CreateTokenParams {
        name, symbol, description, image_uri,
        website: None, twitter: None, telegram: None,
        creator_address: wallet,
        amount_out,                              // client-computed, passed in
        value: initial_buy,
        action_id: ActionId::CapricornActor,
    },
    &api,
).await?;
```

### v2 — `NadFunRouter.create` / `createWithNative`

v2 supports **multiple quote tokens** (WMON, LVMON, ERC-20s); each quote token
has its own genesis `QuoteConfig`. The client does **not** pass `amountOut` — the
router/curve compute `tokenOut` and return it.

`INadFunRouter.CreateParams` (per the integration guide):

```solidity
struct CreateParams {
    string name; string symbol; string tokenURI;
    address quoteToken;          // must be registered in ProtocolManager
    uint16  creatorFeeRate;      // BPS; default allowlist 100/300/500 = 1%/3%/5%
    IBondingCurve.VaultAllocation[] vaults;   // bps must total 10000, ≤ 5 vaults
    bytes32 salt;
    ITokenRegistry.DexType dexType;           // current path: UniswapV2
    uint256 buyQuoteAmount;      // initial buy; 0 = no initial buy
    uint256 deadline;
}
```

- **ERC-20 quote** → `NadFunRouter.create(params)` (approve `deployFee +
  buyQuoteAmount` of the quote token first).
- **Native MON** (quote = WMON/LVMON) → `NadFunRouter.createWithNative{value:
  deployFee + buyQuoteAmount}(params)`. The SDK derives
  `native_value = deploy_fee + buy_quote_amount` automatically.

```rust
let params = V2CreateTokenParams {
    name, symbol, description, image_uri,
    creator_address: wallet,     // must equal the signing wallet
    creator_fee_rate: 100,       // 1% — per-token, stored on the curve
    vaults,                      // bps total 10000
    dex_type: V2DexType::NadFun,
    buy_quote_amount: parse_ether("1")?,
    payment: V2CreatePayment::Native,   // or Erc20 { quote_token }
    deadline: U256::from(deadline),
    ..Default::default()
};
let created = core.v2().create_token(params, &api).await?;   // returns token_address, tx, ...
```

#### v2 initial-buy fee model (important)

The v2 create-time initial buy is **anti-sniping exempt**, but it **does** pay
the curve protocol fee **and the per-token creator fee**. On-chain
`BondingCurve._initialBuy` deducts a single combined rate
`mulDivUp(amountIn, curveProtocolFeeRate + creatorFeeRate, BPS)` (ceil), then
applies the constant-product / supply-cap math.

To estimate the exact `tokenOut` for a v2 initial buy **before** sending the tx,
use the SDK helper — note it takes the **creator fee rate** (a per-token
parameter that is *not* part of the genesis quote config):

```rust
// EXACT to the wei vs the on-chain _initialBuy output.
let out = core.v2()
    .get_initial_buy_amount_out(quote_token, amount_in, creator_fee_rate)
    .await?;
```

- Signature: **`get_initial_buy_amount_out(quote_token, amount_in, creator_fee_rate)`**
  (v2). Contrast v1's parameterless `get_initial_buy_amount_out(amount_in)`.
- It reads the quote token's genesis `QuoteConfig` (per-quote, so `quote_token`
  is required) and subtracts protocol + creator fee.
- It returns `Err` when a positive `amount_in`'s ceil-rounded fees would consume
  the entire quote (the on-chain `_initialBuy` reverts in that case rather than
  minting zero).
- A later (post-creation) buy on the same curve differs by design: it also
  carries the time-decaying anti-sniping penalty that the create-time buy is
  exempt from.

#### v2 deployment addresses (testnet example)

| Name | Address |
|------|---------|
| `WMON` | `0x5a4E0bFDeF88C9032CB4d24338C5EB3d3870BfDd` |
| `V2_NAD_FUN_ROUTER` | `0x75588668999cA0557b78046b8a5E86b47b9234ec` |
| `V2_BONDING_CURVE` | `0x27063a38eC0D3281D354090EB92e669Ed1eB956C` |
| `V2_PROTOCOL_MANAGER` | `0x2F98030aBD7c59e3E5Dc6b4b66b6008821d0fB41` |
| `V2_TOKEN_REGISTRY` | `0x2Bc127be900aD290E703Cd2C71eB0EDCa162C898` |

Addresses differ per environment — the SDK resolves them from
`nadfun_sdk::constants` by `Network`. Confirm production addresses before use.

---

## Step 5: Index

After the create transaction is mined, query the live token:

- `GET /token/:token` — token info (`version: "V1" | "V2"`, `is_graduated`, …).
- `GET /token/metadata/:token_id` — token info + `market_info`
  (`market_type` is `CURVE`/`DEX` for v1, `V2_CURVE`/`V2_DEX` for v2).
- `GET /trade/*` — market, chart, metrics, swap history, holders.

The SDK exposes `core.detect_version(token)` / `core.detect_token_info(token)`
for the on-chain version probe, and curve/DEX indexers under `nadfun_sdk::stream`.

---

## v1 vs v2 — at a glance

| Aspect | v1 | v2 |
|--------|----|----|
| Router | `BondingCurveRouter` + `DexRouter` | single `NadFunRouter` |
| Quote asset | native MON only (one genesis curve) | multiple quote tokens, per-quote `QuoteConfig` |
| Salt `version` | `"V1"` | `"V2"` |
| Initial buy amount | client passes `amountOut` | client passes `buyQuoteAmount`; receives `tokenOut` |
| Creator fee | none on the curve | per-token `creatorFeeRate` (bps), charged on the initial buy |
| `get_initial_buy_amount_out` | `(amount_in)` — Lens passthrough, exact | `(quote_token, amount_in, creator_fee_rate)` — computed, exact, errors if fees consume the quote |
| `actionId` | required (`ActionId`) | removed |
| Post-graduation | external Capricorn CL router | `NadFunPair` + `NadSwapAdapter` |

---

## Important Notes

1. **Sequential off-chain steps** — Step 2 needs `image_uri`, Step 3 needs
   `metadata_uri`; call them promptly (NSFW cache TTL).
2. **Predicted address is not final** — always verify the salt's predicted
   `address` against the on-chain `Create` event / receipt.
3. **Creator must equal the signer** — `msg.sender` becomes the creator on-chain;
   a mismatch with the salt's `creator` predicts the wrong address.
4. **v2 fees** — the initial buy pays protocol + creator fee (anti-sniping
   exempt); use `get_initial_buy_amount_out(quote_token, amount_in,
   creator_fee_rate)` for the exact `tokenOut`.

---

## API Base URLs

| Network | Base URL |
|---------|----------|
| Mainnet | `https://api.nad.fun` |
| Testnet | `https://dev-api.nadapp.net` |

External callers may send requests without an `X-API-Key` (lower rate limit) or
with one (`nadfun_` + 32 chars) for a higher limit.
