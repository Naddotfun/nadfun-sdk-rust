//! SDK-built signing provider with per-account nonce handling that survives
//! failed sends.
//!
//! alloy's recommended fillers take the nonce from a cache concurrently with
//! gas estimation and never give it back, so a send that reverts at
//! `eth_estimateGas` (or that the node rejects) leaves a nonce gap and every
//! later transaction from that provider stalls. Here:
//!
//! - the nonce is assigned only after gas estimation succeeds;
//! - one nonce state exists per (chain id, account) per process, keyed by the
//!   chain id filled into the transaction and shared by every SDK-built signing
//!   provider (`Core::new`, `TokenHelper::new`). It lives for the process, so
//!   it survives a node / devnet reset that keeps the chain id;
//! - sends from one account are serialized: the account lock is held from
//!   nonce assignment until the node answers (each RPC under the lock is
//!   time-bounded), so at most one of our sends per account is in flight;
//! - the chain's mined transaction count (`latest`) is read before every
//!   automatic send (one extra RPC) and after the node refuses one. It moves
//!   the state forward when the account was used elsewhere, and back only when
//!   we are ahead and the count has not changed for [`NONCE_STALL_WINDOW`]
//!   (30 s) since we last sent the nonce the chain is waiting on (sends past a
//!   gap do not count) — that transaction was dropped from the mempool, so the
//!   gap is refilled from the chain count.
//!   Monad's `pending` tag equals `latest`, so the count never includes our
//!   accepted-but-unmined transactions;
//! - after a send: accepted (or, on `eth_sendRawTransactionSync` only, an error
//!   carrying the tx hash) -> n + 1; "nonce too low" / "already known" /
//!   "already imported" / "known transaction" / "replacement transaction
//!   underpriced" -> max(n + 1, chain); any other node error, including "nonce
//!   too high" -> max(n, chain); transport error, timeout or dropped send -> n.
//!
//! Residual risks:
//! 1. Ambiguous outcome (transport error / timeout): the tx may have landed. The
//!    next send reuses its nonce, so only one of the two executes — the caller may
//!    get `Err` for the one that executed, or `Ok(hash)` for one that never mines.
//! 2. A live tx not included for more than 30 s is "presumed dropped": the next
//!    send after stall recovery re-signs its nonce and can replace that slow but
//!    alive tx (same outcome as 1).
//! 3. A tx the node accepted and then dropped: later sends stall for up to 30 s,
//!    then recover by reusing its nonce.
//! 4. First use after a process restart while the previous process still has
//!    unmined txs: those nonces are reused (Monad `pending` == `latest`).
//! 5. Not covered: providers passed to `Core::with_provider`, other processes or
//!    wallets on the same key, a devnet reset that keeps the chain id (repaired by
//!    the 30 s stall recovery).
//! 6. Cost: +1 `eth_getTransactionCount` per automatic send (+1 more after a
//!    refused send); sends are serialized per account process-wide, and a sync
//!    send may hold the account for up to 120 s.
//! 7. Errors are classified by partial message text; unknown wordings fall back
//!    to max(n, chain count).
//! 8. Raw sends (`send_raw_transaction*`) through the same provider are not
//!    intercepted, so the state can be stale afterwards: the next send may fail
//!    once ("nonce too low") and then recover. Envelopes signed by another key and
//!    requests with another `from` pass through without touching the state.

use alloy::{
    consensus::{transaction::SignerRecoverable as _, Transaction as _},
    network::{Ethereum, EthereumWallet, TransactionBuilder},
    primitives::{Address, Bytes, U64},
    providers::{
        fillers::{FillProvider, TxFiller},
        DynProvider, EthCall, PendingTransactionBuilder, Provider, ProviderBuilder, RootProvider,
        SendableTx,
    },
    rpc::types::{TransactionReceipt, TransactionRequest},
    signers::local::PrivateKeySigner,
    transports::{RpcError, TransportErrorKind, TransportResult},
};
use anyhow::Result;
use std::{
    collections::HashMap,
    future::Future,
    pin::Pin,
    sync::{Arc, OnceLock},
    time::{Duration, Instant},
};
use tokio::sync::Mutex;

/// Bound on each RPC made while holding an account lock.
const RPC_TIMEOUT: Duration = Duration::from_secs(30);
/// Bound on `eth_sendRawTransactionSync`, which waits for inclusion.
const SYNC_SEND_TIMEOUT: Duration = Duration::from_secs(120);

/// How long the chain count may stay below our next nonce without moving
/// before our pending transactions are presumed dropped (stall recovery).
pub(crate) const NONCE_STALL_WINDOW: Duration = Duration::from_secs(30);

/// Nonce state of one account.
#[derive(Debug, Default)]
struct AccountState {
    /// Next nonce to hand out; `None` = not read from the chain yet.
    next: Option<u64>,
    /// Last chain count seen and when it last changed.
    chain: Option<(u64, Instant)>,
}

type Slot = Arc<Mutex<AccountState>>;

/// The process-wide nonce state for `address` on `chain_id`.
fn account_slot(chain_id: u64, address: Address) -> Slot {
    static SLOTS: OnceLock<std::sync::Mutex<HashMap<(u64, Address), Slot>>> = OnceLock::new();
    let mut slots = SLOTS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    slots.entry((chain_id, address)).or_default().clone()
}

/// Chain id already filled into the transaction, if any.
fn filled_chain_id(tx: &SendableTx<Ethereum>) -> Option<u64> {
    match tx {
        SendableTx::Builder(req) => req.chain_id,
        SendableTx::Envelope(envelope) => envelope.chain_id(),
    }
}

/// How a filled transaction is handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Route {
    /// Ours, no nonce yet: assign one ([`send_auto`]).
    Auto,
    /// Ours with a caller-chosen nonce ([`send_fixed`]).
    Fixed(u64),
    /// Signed or sent for another account: never touches our state.
    PassThrough,
}

fn route(tx: &SendableTx<Ethereum>, signer: Address) -> Route {
    match tx {
        SendableTx::Builder(req) if req.from.is_some_and(|from| from != signer) => {
            Route::PassThrough
        }
        SendableTx::Builder(req) => req.nonce.map_or(Route::Auto, Route::Fixed),
        SendableTx::Envelope(envelope) => match envelope.recover_signer() {
            Ok(from) if from == signer => Route::Fixed(envelope.nonce()),
            _ => Route::PassThrough,
        },
    }
}

/// What a send's result means for the nonce it used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    /// The node accepted the transaction.
    Sent,
    /// The node says the nonce is already used (or queued).
    Taken,
    /// The node refused the transaction for another reason (including "nonce
    /// too high": a load-balanced RPC may say so for c + 1 while c is accepted
    /// elsewhere, so it never moves the state back).
    Rejected,
    /// Unknown whether the node got it (transport / decode failure, timeout,
    /// dropped send).
    Ambiguous,
}

/// `sync`: the error came from `eth_sendRawTransactionSync`.
fn classify(err: &RpcError<TransportErrorKind>, sync: bool) -> Outcome {
    const TAKEN: [&str; 8] = [
        "nonce too low",
        "nonce is too low",
        "nonce has already been used",
        "already known",
        "already imported",
        "already in mempool",
        "known transaction",
        "replacement transaction underpriced",
    ];
    // EIP-7966 sync sends report "added but not processed in time" with the
    // transaction hash in `data`: the transaction is in. On a normal send a
    // hash in `data` proves nothing.
    if sync && err.tx_hash_data().is_some() {
        return Outcome::Sent;
    }
    match err.as_error_resp() {
        Some(payload) => {
            let message = payload.message.to_lowercase();
            if TAKEN.iter().any(|m| message.contains(m)) {
                Outcome::Taken
            } else {
                Outcome::Rejected
            }
        }
        None => Outcome::Ambiguous,
    }
}

/// Whether the chain count must be re-read after `outcome`.
fn needs_chain_count(outcome: Outcome) -> bool {
    matches!(outcome, Outcome::Taken | Outcome::Rejected)
}

/// Next nonce after sending with `n`; `chain_count` is the fresh count when
/// [`needs_chain_count`]. `None` = arithmetic overflow.
fn next_after(n: u64, outcome: Outcome, chain_count: Option<u64>) -> Option<u64> {
    match outcome {
        Outcome::Sent => n.checked_add(1),
        Outcome::Taken => n.checked_add(1).map(|m| m.max(chain_count.unwrap_or(0))),
        // Reuse `n` unless the chain is already past it (catches "nonce
        // taken" errors in wordings we do not match).
        Outcome::Rejected => Some(chain_count.map_or(n, |c| c.max(n))),
        // Retried with the same nonce: if it did land, the node answers
        // "nonce too low" / "already known" (Taken).
        Outcome::Ambiguous => Some(n),
    }
}

/// Pick the nonce for the next send given the chain count `chain` read just
/// before it: start from the chain on first use, jump to it when someone else
/// moved the account ahead, and fall back to it when we are ahead but the
/// chain has not moved for `window` (our pending transactions were dropped).
fn reconcile(state: &mut AccountState, chain: u64, now: Instant, window: Duration) -> u64 {
    let changed_at = match state.chain {
        Some((seen, at)) if seen == chain => at,
        _ => now,
    };
    state.chain = Some((chain, changed_at));
    let n = match state.next {
        Some(n) if n > chain && now.duration_since(changed_at) < window => n,
        Some(n) if n == chain => n,
        _ => chain,
    };
    state.next = Some(n);
    n
}

/// `fut` bounded by `limit`; a timeout is a transport error (ambiguous).
async fn bounded<T>(
    limit: Duration,
    fut: impl Future<Output = TransportResult<T>>,
) -> TransportResult<T> {
    tokio::time::timeout(limit, fut)
        .await
        .unwrap_or_else(|_| Err(TransportErrorKind::custom_str("RPC timed out")))
}

/// Send with the next automatic nonce, holding the account lock throughout.
async fn send_auto<T, C, CF, S, SF>(
    slot: &Mutex<AccountState>,
    chain_count: C,
    send_timeout: Duration,
    sync: bool,
    clock: impl Fn() -> Instant,
    send: S,
) -> TransportResult<T>
where
    C: Fn() -> CF,
    CF: Future<Output = TransportResult<u64>>,
    S: FnOnce(u64) -> SF,
    SF: Future<Output = TransportResult<T>>,
{
    let mut state = slot.lock().await;
    // One chain read per send drives first use, catch-up and stall recovery.
    let chain = bounded(RPC_TIMEOUT.min(send_timeout), chain_count()).await?;
    // Also written before sending, so a send dropped mid-flight leaves the
    // ambiguous-outcome state: reuse `n`.
    let n = reconcile(&mut state, chain, clock(), NONCE_STALL_WINDOW);
    let sent = bounded(send_timeout, send(n)).await;
    let outcome = match &sent {
        Ok(_) => Outcome::Sent,
        Err(err) => classify(err, sync),
    };
    let fresh = if needs_chain_count(outcome) {
        bounded(RPC_TIMEOUT.min(send_timeout), chain_count())
            .await
            .ok()
    } else {
        None
    };
    if let Some(after) = next_after(n, outcome, fresh) {
        state.next = Some(after);
    }
    // Restart the stall clock only when the node may now hold the nonce the
    // chain is waiting on: sends past a gap must not keep a stall alive.
    if outcome != Outcome::Rejected && n <= chain {
        state.chain = Some((fresh.unwrap_or(chain), clock()));
    }
    sent
}

/// Send a transaction whose nonce the caller chose, holding the account lock;
/// on success move the shared state past it (never backwards).
async fn send_fixed<T, C, CF, S, SF>(
    slot: &Mutex<AccountState>,
    chain_count: C,
    send_timeout: Duration,
    nonce: u64,
    send: S,
) -> TransportResult<T>
where
    C: Fn() -> CF,
    CF: Future<Output = TransportResult<u64>>,
    S: FnOnce() -> SF,
    SF: Future<Output = TransportResult<T>>,
{
    let mut state = slot.lock().await;
    let sent = bounded(send_timeout, send()).await;
    if sent.is_ok() {
        let seen = match state.chain {
            Some((c, _)) => Some(c),
            None => bounded(RPC_TIMEOUT.min(send_timeout), chain_count())
                .await
                .ok(),
        };
        let current = state.next.or(seen);
        if let Some(after) = nonce.checked_add(1) {
            state.next = Some(current.map_or(after, |c| c.max(after)));
        }
        // Restart the stall clock, as for an automatic send — only when this
        // is the nonce the chain is waiting on.
        if let Some(c) = seen.filter(|&c| nonce <= c) {
            state.chain = Some((c, Instant::now()));
        }
    }
    sent
}

/// HTTP provider signing with `signer`: gas / fee / chain-id fillers plus the
/// nonce handling described in the module docs.
pub(crate) fn signing_provider(rpc_url: &str, signer: PrivateKeySigner) -> Result<DynProvider> {
    let address = signer.address();
    let inner = ProviderBuilder::new()
        .disable_recommended_fillers()
        .with_gas_estimation()
        .fetch_chain_id()
        .wallet(EthereumWallet::from(signer))
        .connect_http(rpc_url.parse()?);
    Ok(DynProvider::new(NonceGuard {
        inner,
        signer: address,
    }))
}

struct NonceGuard<F: TxFiller<Ethereum>> {
    /// Fillers without a nonce filler: `fill` leaves the nonce unset.
    inner: FillProvider<F, RootProvider>,
    signer: Address,
}

type Boxed<'a, T> = Pin<Box<dyn Future<Output = TransportResult<T>> + Send + 'a>>;

impl<F: TxFiller<Ethereum>> NonceGuard<F> {
    /// Mined transaction count (`latest`; on Monad `pending` is the same).
    async fn chain_count(&self) -> TransportResult<u64> {
        self.inner.get_transaction_count(self.signer).latest().await
    }

    /// Route `tx` through the nonce rules, then hand it to `send`.
    async fn send<T, S, SF>(
        &self,
        tx: SendableTx<Ethereum>,
        send_timeout: Duration,
        sync: bool,
        send: S,
    ) -> TransportResult<T>
    where
        S: FnOnce(SendableTx<Ethereum>) -> SF,
        SF: Future<Output = TransportResult<T>>,
    {
        // Gas estimation and chain id happen here, before any nonce is taken.
        let tx = match tx {
            SendableTx::Builder(req) => self.inner.fill(req).await?,
            envelope => envelope,
        };
        let chain_id = match filled_chain_id(&tx) {
            Some(id) => id,
            // Only a pre-EIP-155 envelope carries no chain id.
            None => self.inner.get_chain_id().await?,
        };
        let slot = account_slot(chain_id, self.signer);
        let chain_count = || self.chain_count();
        match (route(&tx, self.signer), tx) {
            (Route::Auto, SendableTx::Builder(mut req)) => {
                send_auto(&slot, chain_count, send_timeout, sync, Instant::now, |n| {
                    req.set_nonce(n);
                    send(SendableTx::Builder(req))
                })
                .await
            }
            (Route::Fixed(nonce), tx) => {
                send_fixed(&slot, chain_count, send_timeout, nonce, || send(tx)).await
            }
            (_, tx) => send(tx).await,
        }
    }
}

impl<F: TxFiller<Ethereum> + 'static> Provider for NonceGuard<F> {
    fn root(&self) -> &RootProvider {
        self.inner.root()
    }

    fn call(&self, tx: TransactionRequest) -> EthCall<Ethereum, Bytes> {
        self.inner.call(tx)
    }

    fn estimate_gas(&self, tx: TransactionRequest) -> EthCall<Ethereum, U64, u64> {
        self.inner.estimate_gas(tx)
    }

    // `Provider` is an `#[async_trait]`; these are its desugared signatures.
    fn send_transaction_internal<'a, 'async_trait>(
        &'a self,
        tx: SendableTx<Ethereum>,
    ) -> Boxed<'async_trait, PendingTransactionBuilder<Ethereum>>
    where
        'a: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(self.send(tx, RPC_TIMEOUT, false, |tx| {
            self.inner.send_transaction_internal(tx)
        }))
    }

    fn send_transaction_sync_internal<'a, 'async_trait>(
        &'a self,
        tx: SendableTx<Ethereum>,
    ) -> Boxed<'async_trait, TransactionReceipt>
    where
        'a: 'async_trait,
        Self: 'async_trait,
    {
        Box::pin(self.send(tx, SYNC_SEND_TIMEOUT, true, |tx| {
            self.inner.send_transaction_sync_internal(tx)
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // alloy 2.x moved `build` (sign into an envelope) to the network-specific trait.
    use alloy::network::NetworkTransactionBuilder as _;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    use std::time::Duration;

    const T: Duration = Duration::from_secs(5);
    const STALL: Duration = Duration::from_secs(30);

    fn resp_with(message: &str, data: Option<&str>) -> RpcError<TransportErrorKind> {
        let mut payload = serde_json::json!({ "code": -32000, "message": message });
        if let Some(data) = data {
            payload["data"] = serde_json::json!(data);
        }
        RpcError::ErrorResp(serde_json::from_value(payload).unwrap())
    }

    fn resp(message: &str) -> RpcError<TransportErrorKind> {
        resp_with(message, None)
    }

    fn transport() -> RpcError<TransportErrorKind> {
        RpcError::Transport(TransportErrorKind::BackendGone)
    }

    async fn chain(n: u64) -> TransportResult<u64> {
        Ok(n)
    }

    fn st(next: Option<u64>) -> Mutex<AccountState> {
        Mutex::new(AccountState {
            next,
            ..Default::default()
        })
    }

    /// Chain reads answering `values` in order (the last one repeats).
    fn seq(values: &[u64]) -> impl Fn() -> std::future::Ready<TransportResult<u64>> {
        let values = std::sync::Mutex::new(values.to_vec());
        move || {
            let mut v = values.lock().unwrap();
            let next = if v.len() > 1 { v.remove(0) } else { v[0] };
            std::future::ready(Ok(next))
        }
    }

    async fn never<T>() -> TransportResult<T> {
        std::future::pending().await
    }

    #[test]
    fn classifies_taken_nonce_messages() {
        for msg in [
            "nonce too low",
            "Nonce too low: next nonce 7, tx nonce 5",
            "already known",
            "known transaction: 0xabc",
            "transaction already imported",
            "already in mempool",
            "nonce is too low",
            "Nonce has already been used",
            "replacement transaction underpriced",
        ] {
            assert_eq!(classify(&resp(msg), false), Outcome::Taken, "{msg}");
        }
    }

    #[test]
    fn nonce_too_high_is_a_rejection() {
        // A load-balanced RPC may call c + 1 "too high" while c is accepted on
        // another node: never move back on it (only the stall window does).
        assert_eq!(classify(&resp("NONCE TOO HIGH"), false), Outcome::Rejected);
        assert_eq!(
            classify(&resp("nonce too high: 9 > 4"), false),
            Outcome::Rejected
        );
    }

    #[test]
    fn tx_hash_in_error_data_means_sent_on_sync_only() {
        let hash = format!("0x{}", "ab".repeat(32));
        assert_eq!(
            classify(
                &resp_with("transaction added but not processed in time", Some(&hash)),
                true
            ),
            Outcome::Sent
        );
        // On a normal send a hash in `data` proves nothing (alloy's own
        // example is "insufficient funds" + hash).
        assert_eq!(
            classify(&resp_with("insufficient funds", Some(&hash)), false),
            Outcome::Rejected
        );
        // Non-hash data does not count.
        assert_eq!(
            classify(&resp_with("execution reverted", Some("0x716b69e1")), false),
            Outcome::Rejected
        );
    }

    #[test]
    fn classifies_other_rejections_and_ambiguous_errors() {
        for msg in [
            "Insufficient funds for gas * price + value",
            "execution reverted",
            "intrinsic gas too low",
            "rate limit exceeded",
            "invalid transaction nonce", // unknown wording: Rejected + chain resync
            "invalid nonce",             // some nodes say this for a too-high / gap nonce
        ] {
            assert_eq!(classify(&resp(msg), false), Outcome::Rejected, "{msg}");
        }
        assert_eq!(classify(&transport(), false), Outcome::Ambiguous);
        assert_eq!(classify(&RpcError::NullResp, false), Outcome::Ambiguous);
    }

    #[test]
    fn next_nonce_per_outcome() {
        assert_eq!(next_after(5, Outcome::Sent, None), Some(6));
        assert_eq!(next_after(5, Outcome::Ambiguous, None), Some(5));
        // Rejected: reuse n, unless the chain is already past it.
        assert_eq!(next_after(5, Outcome::Rejected, Some(3)), Some(5));
        assert_eq!(next_after(5, Outcome::Rejected, Some(8)), Some(8));
        assert_eq!(next_after(5, Outcome::Rejected, None), Some(5));
        // Taken: past n, and never below a fresher chain count.
        assert_eq!(next_after(5, Outcome::Taken, Some(3)), Some(6));
        assert_eq!(next_after(5, Outcome::Taken, Some(9)), Some(9));
        assert_eq!(next_after(5, Outcome::Taken, None), Some(6));
        // checked arithmetic
        assert_eq!(next_after(u64::MAX, Outcome::Sent, None), None);
        assert_eq!(next_after(u64::MAX, Outcome::Taken, Some(1)), None);
    }

    #[tokio::test]
    async fn auto_nonce_state_machine() {
        // Each send reads the chain once before sending (`pre`) and, for
        // refused sends, once after (`post`): `seq(&[pre, post])`.
        let slot = st(None);
        // First use starts at the chain count; Ok advances.
        let r: TransportResult<u64> =
            send_auto(&slot, seq(&[4]), T, false, Instant::now, |n| async move {
                Ok(n)
            })
            .await;
        assert_eq!(r.unwrap(), 4);
        assert_eq!(slot.lock().await.next, Some(5));
        // Chain behind our state but not stalled: keep our nonce.
        let r: TransportResult<u64> =
            send_auto(&slot, seq(&[4]), T, false, Instant::now, |n| async move {
                Ok(n)
            })
            .await;
        assert_eq!(r.unwrap(), 5);
        assert_eq!(slot.lock().await.next, Some(6));
        // Rejected: reuse, but follow a chain count already past n.
        let _ = send_auto(&slot, seq(&[5, 5]), T, false, Instant::now, |_| async {
            Err::<(), _>(resp("insufficient funds"))
        })
        .await;
        assert_eq!(slot.lock().await.next, Some(6));
        let _ = send_auto(&slot, seq(&[6, 9]), T, false, Instant::now, |_| async {
            Err::<(), _>(resp("invalid transaction nonce"))
        })
        .await;
        assert_eq!(slot.lock().await.next, Some(9));
        // Ambiguous: reuse.
        let _ = send_auto(&slot, seq(&[9]), T, false, Instant::now, |_| async {
            Err::<(), _>(transport())
        })
        .await;
        assert_eq!(slot.lock().await.next, Some(9));
        // Taken: max(n + 1, fresh chain count).
        let _ = send_auto(&slot, seq(&[9, 12]), T, false, Instant::now, |_| async {
            Err::<(), _>(resp("nonce too low"))
        })
        .await;
        assert_eq!(slot.lock().await.next, Some(12));
        let _ = send_auto(&slot, seq(&[12, 2]), T, false, Instant::now, |_| async {
            Err::<(), _>(resp("already known"))
        })
        .await;
        assert_eq!(slot.lock().await.next, Some(13));
        // Too high: like any rejection — never moves back.
        let _ = send_auto(&slot, seq(&[12, 4]), T, false, Instant::now, |_| async {
            Err::<(), _>(resp("nonce too high"))
        })
        .await;
        assert_eq!(slot.lock().await.next, Some(13));
    }

    /// Security regression (H-1): our own accepted sends restart the stall
    /// clock. Rejected at t0, idle > window, then send A and B back to back
    /// while `latest` still lags: B must get A's nonce + 1, not A's nonce.
    #[tokio::test]
    async fn own_accepted_send_restarts_stall_clock() {
        let c = 10;
        let slot = Mutex::new(AccountState {
            next: Some(c),
            chain: Some((c, Instant::now() - Duration::from_secs(60))),
        });
        let a: TransportResult<u64> =
            send_auto(&slot, seq(&[c]), T, false, Instant::now, |n| async move {
                Ok(n)
            })
            .await;
        let b: TransportResult<u64> =
            send_auto(&slot, seq(&[c]), T, false, Instant::now, |n| async move {
                Ok(n)
            })
            .await;
        assert_eq!(a.unwrap(), c);
        assert_eq!(b.unwrap(), c + 1, "B reused A's nonce");
        assert_eq!(slot.lock().await.next, Some(c + 2));
    }

    #[tokio::test]
    async fn tx_hash_error_counts_as_sent_only_on_sync_path() {
        let hash = format!("0x{}", "cd".repeat(32));
        let slot = st(Some(5));
        let h = hash.clone();
        let _ = send_auto(&slot, seq(&[5]), T, false, Instant::now, |_| async move {
            Err::<(), _>(resp_with("insufficient funds", Some(&h)))
        })
        .await;
        assert_eq!(slot.lock().await.next, Some(5), "normal send: reuse");
        let _ = send_auto(&slot, seq(&[5]), T, true, Instant::now, |_| async move {
            Err::<(), _>(resp_with("not processed in time", Some(&hash)))
        })
        .await;
        assert_eq!(slot.lock().await.next, Some(6), "sync send: it is in");
    }

    /// Security regression (v6): sends past a stuck nonce must not keep the
    /// stall clock alive. Tx `c` was dropped, the chain is stuck at `c`, and a
    /// busy bot keeps sending every 10 s: recovery to `c` must still happen
    /// once the window passes since `c` was last sent.
    #[tokio::test]
    async fn gap_sends_do_not_restart_stall_clock() {
        let c = 10;
        let base = Instant::now();
        let now = std::sync::Mutex::new(base);
        let slot = Mutex::new(AccountState {
            next: Some(c + 1),
            chain: Some((c, base)),
        });
        let mut nonces = Vec::new();
        for _ in 0..6 {
            *now.lock().unwrap() += Duration::from_secs(10);
            let clock = || *now.lock().unwrap();
            let sent: TransportResult<u64> =
                send_auto(&slot, seq(&[c]), T, false, clock, |n| async move { Ok(n) }).await;
            nonces.push(sent.unwrap());
        }
        // t = 10, 20 gap sends; t = 30 recovers `c`; t = 60 recovers again.
        assert_eq!(nonces, [c + 1, c + 2, c, c + 1, c + 2, c]);
    }

    /// A caller-supplied gap nonce does not restart the stall clock either.
    #[tokio::test]
    async fn caller_gap_nonce_does_not_restart_stall_clock() {
        let c = 10;
        let slot = Mutex::new(AccountState {
            next: Some(c + 1),
            chain: Some((c, Instant::now() - Duration::from_secs(60))),
        });
        let _ = send_fixed(&slot, seq(&[c]), T, c + 1, || async { Ok(()) }).await;
        let next: TransportResult<u64> =
            send_auto(&slot, seq(&[c]), T, false, Instant::now, |n| async move {
                Ok(n)
            })
            .await;
        assert_eq!(next.unwrap(), c, "stall recovery suppressed by a gap send");
    }

    /// The same restart applies to a caller-supplied nonce.
    #[tokio::test]
    async fn caller_nonce_send_restarts_stall_clock() {
        let c = 10;
        let slot = Mutex::new(AccountState {
            next: Some(c),
            chain: Some((c, Instant::now() - Duration::from_secs(60))),
        });
        let _ = send_fixed(&slot, seq(&[c]), T, c, || async { Ok(()) }).await;
        let b: TransportResult<u64> =
            send_auto(&slot, seq(&[c]), T, false, Instant::now, |n| async move {
                Ok(n)
            })
            .await;
        assert_eq!(b.unwrap(), c + 1, "auto send reused the caller's nonce");
    }

    #[test]
    fn reconcile_keeps_state_while_chain_advances() {
        let t0 = Instant::now();
        let mut s = AccountState {
            next: Some(10),
            ..Default::default()
        };
        // Our txs are pending and the chain keeps moving: never resync, even
        // long after the first observation.
        assert_eq!(reconcile(&mut s, 7, t0, STALL), 10);
        assert_eq!(reconcile(&mut s, 8, t0 + STALL, STALL), 10);
        assert_eq!(reconcile(&mut s, 9, t0 + STALL * 2, STALL), 10);
        // Below the window since the last change: still keep.
        assert_eq!(
            reconcile(&mut s, 9, t0 + STALL * 3 - Duration::from_secs(1), STALL),
            10
        );
    }

    #[test]
    fn reconcile_resyncs_after_stall_window() {
        let t0 = Instant::now();
        let mut s = AccountState {
            next: Some(10),
            ..Default::default()
        };
        assert_eq!(reconcile(&mut s, 7, t0, STALL), 10);
        // Chain stuck at 7 for the whole window while we are ahead: our
        // pending txs were dropped — restart from the chain count.
        assert_eq!(reconcile(&mut s, 7, t0 + STALL, STALL), 7);
        assert_eq!(s.next, Some(7));
    }

    #[test]
    fn reconcile_jumps_to_chain_ahead_and_inits() {
        let t0 = Instant::now();
        let mut s = AccountState {
            next: Some(3),
            ..Default::default()
        };
        // Someone else used the account: jump forward immediately.
        assert_eq!(reconcile(&mut s, 8, t0, STALL), 8);
        // Uninitialised: start at the chain count.
        let mut fresh = AccountState::default();
        assert_eq!(reconcile(&mut fresh, 5, t0, STALL), 5);
        // Equal: keep.
        assert_eq!(reconcile(&mut fresh, 5, t0 + STALL * 5, STALL), 5);
    }

    #[tokio::test]
    async fn dropped_send_reuses_nonce() {
        // Uninitialised: the nonce read from the chain must stick even though
        // the send never completes.
        let slot = st(None);
        let pending = send_auto(
            &slot,
            || chain(7),
            T,
            false,
            Instant::now,
            |_| never::<()>(),
        );
        assert!(tokio::time::timeout(Duration::from_millis(20), pending)
            .await
            .is_err());
        assert_eq!(slot.lock().await.next, Some(7));
    }

    #[tokio::test]
    async fn timed_out_send_is_ambiguous() {
        let slot = st(Some(3));
        let r = send_auto(
            &slot,
            || chain(0),
            Duration::from_millis(20),
            false,
            Instant::now,
            |_| never::<()>(),
        )
        .await;
        assert!(r.is_err(), "a stuck send must time out");
        assert_eq!(slot.lock().await.next, Some(3));
        // The lock is free again.
        let r: TransportResult<u64> = send_auto(
            &slot,
            || chain(0),
            T,
            false,
            Instant::now,
            |n| async move { Ok(n) },
        )
        .await;
        assert_eq!(r.unwrap(), 3);
    }

    #[tokio::test]
    async fn timed_out_chain_read_fails_without_state() {
        let slot = st(None);
        let r: TransportResult<u64> = send_auto(
            &slot,
            never::<u64>,
            Duration::from_millis(20),
            false,
            Instant::now,
            |n| async move { Ok(n) },
        )
        .await;
        assert!(r.is_err());
        assert_eq!(slot.lock().await.next, None);
    }

    #[tokio::test]
    async fn sends_are_serialized_per_account() {
        let slot = Arc::new(st(Some(0)));
        let in_flight = Arc::new(AtomicUsize::new(0));
        let send = |slot: Arc<Mutex<AccountState>>, in_flight: Arc<AtomicUsize>| async move {
            send_auto(
                &slot,
                || chain(0),
                T,
                false,
                Instant::now,
                |n| async move {
                    assert_eq!(
                        in_flight.fetch_add(1, Ordering::SeqCst),
                        0,
                        "overlapping sends"
                    );
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    in_flight.fetch_sub(1, Ordering::SeqCst);
                    Ok(n)
                },
            )
            .await
        };
        let (a, b) = tokio::join!(
            send(slot.clone(), in_flight.clone()),
            send(slot.clone(), in_flight.clone())
        );
        let mut got = [a.unwrap(), b.unwrap()];
        got.sort();
        assert_eq!(got, [0, 1]);
        assert_eq!(slot.lock().await.next, Some(2));
    }

    #[tokio::test]
    async fn caller_nonce_advances_shared_state() {
        let slot = st(Some(3));
        let _ = send_fixed(&slot, || chain(0), T, 8, || async { Ok(()) }).await;
        assert_eq!(slot.lock().await.next, Some(9));
        // A lower caller nonce never moves the state back; failures leave it.
        let _ = send_fixed(&slot, || chain(0), T, 2, || async { Ok(()) }).await;
        assert_eq!(slot.lock().await.next, Some(9));
        let _ = send_fixed(
            &slot,
            || chain(0),
            T,
            20,
            || async { Err::<(), _>(resp("insufficient funds")) },
        )
        .await;
        assert_eq!(slot.lock().await.next, Some(9));
        // A stuck caller-nonce send times out and leaves the state.
        let r = send_fixed(
            &slot,
            || chain(0),
            Duration::from_millis(20),
            30,
            never::<()>,
        )
        .await;
        assert!(r.is_err());
        assert_eq!(slot.lock().await.next, Some(9));
        // Uninitialised state starts from the chain count.
        let fresh = st(None);
        let _ = send_fixed(&fresh, || chain(12), T, 4, || async { Ok(()) }).await;
        assert_eq!(fresh.lock().await.next, Some(12));
    }

    // ---- routing through a real `NonceGuard` over a mocked transport ----

    use alloy::transports::mock::Asserter;

    /// `signing_provider`'s filler stack over a mocked transport.
    fn mocked(asserter: Asserter, signer: PrivateKeySigner) -> DynProvider {
        let address = signer.address();
        let inner = ProviderBuilder::new()
            .disable_recommended_fillers()
            .with_gas_estimation()
            .fetch_chain_id()
            .wallet(EthereumWallet::from(signer))
            .connect_mocked_client(asserter);
        DynProvider::new(NonceGuard {
            inner,
            signer: address,
        })
    }

    /// Fully specified request (no filler RPCs needed), chain id 10143.
    fn full_request(nonce: Option<u64>) -> TransactionRequest {
        let req = TransactionRequest::default()
            .with_chain_id(10143)
            .with_gas_limit(21_000)
            .with_max_fee_per_gas(1)
            .with_max_priority_fee_per_gas(1)
            .with_to(Address::repeat_byte(3));
        match nonce {
            Some(n) => req.with_nonce(n),
            None => req,
        }
    }

    async fn state_of(address: Address) -> Option<u64> {
        account_slot(10143, address).lock().await.next
    }

    fn hash() -> alloy::primitives::B256 {
        alloy::primitives::B256::repeat_byte(0xaa)
    }

    #[tokio::test]
    async fn routes_by_signer_and_nonce() {
        let signer = PrivateKeySigner::random();
        let me = signer.address();
        let mine = |req: TransactionRequest| SendableTx::Builder(req.with_from(me));
        assert_eq!(route(&mine(full_request(None)), me), Route::Auto);
        assert_eq!(route(&mine(full_request(Some(4))), me), Route::Fixed(4));
        let foreign = full_request(None).with_from(Address::repeat_byte(9));
        assert_eq!(route(&SendableTx::Builder(foreign), me), Route::PassThrough);
        let own = full_request(Some(7))
            .build(&EthereumWallet::from(signer))
            .await
            .unwrap();
        assert_eq!(route(&SendableTx::Envelope(own), me), Route::Fixed(7));
        let theirs = full_request(Some(7))
            .build(&EthereumWallet::from(PrivateKeySigner::random()))
            .await
            .unwrap();
        assert_eq!(route(&SendableTx::Envelope(theirs), me), Route::PassThrough);
    }

    #[tokio::test]
    async fn own_envelope_goes_through_fixed_path() {
        let signer = PrivateKeySigner::random();
        let me = signer.address();
        let envelope = full_request(Some(7))
            .build(&EthereumWallet::from(signer.clone()))
            .await
            .unwrap();
        let asserter = Asserter::new();
        asserter.push_success(&hash()); // eth_sendRawTransaction
        asserter.push_success(&U64::from(3)); // chain count
        let _ = mocked(asserter, signer)
            .send_tx_envelope(envelope)
            .await
            .unwrap();
        // Advanced past the caller's nonce although nothing is mined.
        assert_eq!(state_of(me).await, Some(8));
    }

    #[tokio::test]
    async fn explicit_nonce_builder_goes_through_fixed_path() {
        let signer = PrivateKeySigner::random();
        let me = signer.address();
        let asserter = Asserter::new();
        asserter.push_success(&hash());
        asserter.push_success(&U64::from(3));
        let _ = mocked(asserter, signer)
            .send_transaction(full_request(Some(5)))
            .await
            .unwrap();
        assert_eq!(state_of(me).await, Some(6));
    }

    #[tokio::test]
    async fn foreign_envelope_leaves_state_alone() {
        let signer = PrivateKeySigner::random();
        let me = signer.address();
        let other = PrivateKeySigner::random();
        let envelope = full_request(Some(7))
            .build(&EthereumWallet::from(other.clone()))
            .await
            .unwrap();
        let asserter = Asserter::new();
        asserter.push_success(&hash());
        asserter.push_success(&U64::from(3));
        let _ = mocked(asserter, signer)
            .send_tx_envelope(envelope)
            .await
            .unwrap();
        assert_eq!(state_of(me).await, None, "our state moved");
        assert_eq!(state_of(other.address()).await, None, "their state moved");
    }

    #[tokio::test]
    async fn foreign_from_builder_leaves_state_alone() {
        let signer = PrivateKeySigner::random();
        let me = signer.address();
        let asserter = Asserter::new();
        asserter.push_success(&U64::from(3));
        let req = full_request(None).with_from(Address::repeat_byte(9));
        // Our wallet cannot sign for another `from`: the send fails locally.
        assert!(mocked(asserter, signer)
            .send_transaction(req)
            .await
            .is_err());
        assert_eq!(state_of(me).await, None, "our state moved");
    }

    #[tokio::test]
    async fn sync_send_reaches_the_guard() {
        let signer = PrivateKeySigner::random();
        let me = signer.address();
        let receipt = serde_json::json!({
            "type": "0x2",
            "status": "0x1",
            "cumulativeGasUsed": "0x5208",
            "logs": [],
            "logsBloom": format!("0x{}", "00".repeat(256)),
            "transactionHash": hash(),
            "transactionIndex": "0x0",
            "blockHash": alloy::primitives::B256::repeat_byte(0xbb),
            "blockNumber": "0x1",
            "gasUsed": "0x5208",
            "effectiveGasPrice": "0x1",
            "from": me,
            "to": Address::repeat_byte(3),
            "contractAddress": null
        });
        let asserter = Asserter::new();
        asserter.push_success(&receipt); // eth_sendRawTransactionSync
        asserter.push_success(&U64::from(3));
        mocked(asserter, signer)
            .send_transaction_sync(full_request(Some(9)))
            .await
            .unwrap();
        assert_eq!(state_of(me).await, Some(10), "sync send bypassed the guard");
    }

    #[test]
    fn one_state_per_chain_and_account() {
        let a = Address::repeat_byte(1);
        assert!(Arc::ptr_eq(&account_slot(1, a), &account_slot(1, a)));
        assert!(!Arc::ptr_eq(&account_slot(1, a), &account_slot(2, a)));
        assert!(!Arc::ptr_eq(
            &account_slot(1, a),
            &account_slot(1, Address::repeat_byte(2))
        ));
    }

    #[tokio::test]
    async fn slot_key_uses_the_filled_chain_id() {
        let req = TransactionRequest::default()
            .with_chain_id(10143)
            .with_nonce(0)
            .with_gas_limit(21_000)
            .with_max_fee_per_gas(1)
            .with_max_priority_fee_per_gas(1)
            .with_to(Address::repeat_byte(3));
        assert_eq!(
            filled_chain_id(&SendableTx::Builder(req.clone())),
            Some(10143)
        );
        let wallet = EthereumWallet::from(PrivateKeySigner::random());
        let envelope = req.build(&wallet).await.unwrap();
        assert_eq!(
            filled_chain_id(&SendableTx::Envelope(envelope)),
            Some(10143)
        );
        let unfilled = TransactionRequest::default();
        assert_eq!(filled_chain_id(&SendableTx::Builder(unfilled)), None);
    }
}
