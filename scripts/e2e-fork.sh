#!/usr/bin/env bash
# Monad TESTNET anvil-fork E2E for v2 multi-quote trading.
#
# Starts a pinned fork on a free local port, funds a throwaway key (MON via
# anvil_setBalance, USDC via its balance storage slot), runs
# tests/e2e_fork_multi_quote.rs, and leaves the report at target/e2e-fork-report.json.
#
# Env: FORK_URL (default https://dev-node.nadapp.net), FORK_BLOCK (pinned),
#      PORT (default 18645), E2E_API_URL (optional api-server override).
set -euo pipefail

FORK_URL=${FORK_URL:-https://dev-node.nadapp.net}
FORK_BLOCK=${FORK_BLOCK:-71275425}
PORT=${PORT:-18645}
USDC=0x17b032Dc6B33D54328E4F6518852F3F3DB166d0E
ROOT=$(cd "$(dirname "$0")/.." && pwd)
RPC="http://127.0.0.1:$PORT"

mkdir -p "$ROOT/target"
if lsof -nP -iTCP:"$PORT" -sTCP:LISTEN >/dev/null 2>&1; then
  echo "port $PORT already in use; pick another PORT" >&2
  exit 1
fi

anvil --fork-url "$FORK_URL" --fork-block-number "$FORK_BLOCK" --port "$PORT" \
  --silent >"$ROOT/target/e2e-fork-anvil.log" 2>&1 &
ANVIL_PID=$!
trap 'kill $ANVIL_PID 2>/dev/null || true' EXIT
for _ in $(seq 1 60); do
  cast chain-id --rpc-url "$RPC" >/dev/null 2>&1 && break
  sleep 0.5
done
# Make sure the node answering on $PORT is the anvil we just started, forked at our pin.
kill -0 "$ANVIL_PID" 2>/dev/null || { echo "anvil exited; see target/e2e-fork-anvil.log" >&2; exit 1; }
[ "$(cast chain-id --rpc-url "$RPC")" = "10143" ] || { echo "fork not testnet" >&2; exit 1; }
[ "$(cast rpc --rpc-url "$RPC" anvil_nodeInfo | jq -r '.forkConfig.forkBlockNumber')" = "$FORK_BLOCK" ] ||
  { echo "node on $PORT is not our fork at block $FORK_BLOCK" >&2; exit 1; }

# Throwaway key for this fork only; never printed.
PK=0x$(openssl rand -hex 32)
WALLET=$(cast wallet address --private-key "$PK")
cast rpc --rpc-url "$RPC" anvil_setBalance "$WALLET" 0x84595161401484a000000 >/dev/null # 10M MON

# USDC balance: probe mapping slots 0..20 until balanceOf reflects the write.
WANT=10000000000000 # 10,000,000 USDC (6 decimals)
WANT_HEX=$(cast to-uint256 "$WANT")
FUNDED=
for slot in $(seq 0 20); do
  key=$(cast index address "$WALLET" "$slot")
  cast rpc --rpc-url "$RPC" anvil_setStorageAt "$USDC" "$key" "$WANT_HEX" >/dev/null
  cast rpc --rpc-url "$RPC" evm_mine >/dev/null
  if [ "$(cast call --rpc-url "$RPC" "$USDC" 'balanceOf(address)(uint256)' "$WALLET" | cut -d' ' -f1)" = "$WANT" ]; then
    FUNDED=$slot
    break
  fi
  cast rpc --rpc-url "$RPC" anvil_setStorageAt "$USDC" "$key" "$(cast to-uint256 0)" >/dev/null
done
[ -n "$FUNDED" ] || { echo "could not locate USDC balance slot" >&2; exit 1; }
echo "fork block $FORK_BLOCK on $RPC; wallet $WALLET funded (USDC slot $FUNDED)"

cd "$ROOT"
E2E_RPC_URL="$RPC" E2E_PRIVATE_KEY="$PK" E2E_FORK_BLOCK="$FORK_BLOCK" E2E_REPORT_PATH="$ROOT/target/e2e-fork-report.json" \
  cargo test --test e2e_fork_multi_quote -- --ignored --nocapture
jq '{fork_block_pin, all_pass, scenarios: [.scenarios[] | {name, result}]}' \
  "$ROOT/target/e2e-fork-report.json"
