# TODO

## ▶ CURRENT — v2 namespace refactor + v1↔v2 parity
Spec: `docs/superpowers/specs/2026-05-30-v2-namespace-refactor-design.md` · 3 PRs off `v2`.

### Phase 1 — namespace refactor (breaking) — `feat/v2-namespace`
- [ ] `CoreV1<'a>` / `CoreV2<'a>` handles + `core.v1()` / `core.v2()` accessors
- [ ] Move v1 → `src/core/v1/handle.rs`; v2 → `src/core/v2/handle.rs` (drop `_v2`)
- [ ] Cross-cutting (`detect_*`, `get_receipt`, accessors) stay on `Core`
- [ ] `lib.rs` + `prelude` re-export `CoreV1`, `CoreV2`
- [ ] ~97 call sites: examples + tests + README + EXAMPLES.md + llms.txt + CHANGELOG
- [ ] Dispatch tests (TDD) + `cargo build --examples` + clippy gate + `/codex review` → PR

### Phase 2 — v2 passthrough parity (additive) — `feat/v2-passthrough-parity`
- [ ] **R1: pin deployed contract variant via live testnet `eth_call`** (blocker)
- [ ] `V2Curve`, `V2QuoteConfig` types; add `getCurve` wrapper; extend ProtocolManager binding
- [ ] `is_halted`, `get_sniping_penalty`, `get_curve`, `quote_token`, `quote_config`, `is_locked` (caveat), `get_reserves`, `get_dex_type`, `is_registered`
- [ ] Tests + docs (additive) + `/codex review` → PR

### Phase 3 — v2 computed helpers (additive) — `feat/v2-computed-parity`
- [ ] **R2: pin exact fee basis from `BondingCurveLibrary`/`_initialBuy`**
- [ ] `get_progress`, `available_buy_tokens`, `get_initial_buy_amount_out(quote_token, amount_in)`
- [ ] Drift tests: SDK math vs on-chain at pinned testnet block (Rule 9) + `/codex review` → PR

---

## (superseded) v2 branch wrap-up
> Below predates the unified-core merge (PR #2). Unified `Core` + `with_provider`
> + `set_network` removal + Codex P1/P2/P3 are **done** (see
> `docs/superpowers/plans/2026-05-30-unified-core-refactor.md`). Kept for the
> live-env reference at the bottom; the open items here are largely obsolete.

> **Branch**: `v2` (14 commits ahead of `main`, last commit `5fc1275`)
> **Status**: 동작/테스트/스모크 통과, **머지 불가** — Codex P1 미처리 + 통합 Core 리팩터 필요

## 다음 세션 핵심 방향

**`Core`와 `CoreV2`를 통합한다.** 단일 `Core`가 v1·v2 토큰 모두 거래/생성/스트림 가능하도록.
- 사용자는 `Core` 하나만 신경 쓰면 됨 (현재의 "분리 제공" 결정 → "통합" 재선회)
- v1 기존 시그니처/이름은 0.4.0 minor breaking으로 변경 허용 (set_network 제거 + Network 인스턴스 필드)
- v2 전용 메서드는 `Core` 위에 `*_v2` 또는 내부 자동 분기 형태로 노출 — 구체 결정은 다음 세션 도입부에서

권장 시작 명령:
```bash
git checkout v2
cat ~/.claude/plans/nadfun-v2-next-session.md  # 풀 핸드오프
```

---

## 작업 목록

### 1. v1/v2 단일 Core 통합 (메인 작업)

- [ ] `Core` 단일 진입점으로 통합 — `CoreV2` 제거하고 기능 흡수
- [ ] `set_network` 전역 lock 제거 → `Network`를 `Core` 인스턴스 필드로
- [ ] `Core::new(rpc, key, network)` 시그니처 유지 (이미 network 받음). 내부 동작만 정리.
- [ ] `Core::with_provider(provider, wallet, network)` 추가 — 같은 provider/wallet으로 같은 인스턴스에서 v1+v2 거래
- [ ] 토큰별 v1/v2 자동 분기: `TokenRegistryV2::is_registered(token)` on-chain probe 또는 `ApiClient::get_token(token).version` 기반 (캐시 포함)
- [ ] v2 전용 기능(exact-out, permit + quote_token, vault create) 노출 방식 결정:
  - 옵션 A) `core.exact_out_buy(...)` 같은 추가 메서드만 — vanilla buy/sell은 자동 분기
  - 옵션 B) 모든 v2 전용은 `core.v2_*` 접두사
  - 옵션 C) `core.v2_engine()` escape hatch + 자동 분기
- [ ] `Router` enum에 `NadFun` variant 추가 검토 (외부 match 깨질 수 있음 → `#[non_exhaustive]`)

### 2. v1 free-function/생성자 시그니처 마이그레이션 (`set_network` 제거 후속)

- [ ] `ApiClient::new()` → `ApiClient::new(network)` (1-arg breaking)
- [ ] `ApiClient::from_env()` → `from_env(network)` 또는 `from_env()` + global 폐기
- [ ] `CurveStream::new(url)` → `CurveStream::new(url, network)`
- [ ] `CurveIndexer::new(provider)` → `CurveIndexer::new(provider, network)`
- [ ] `DexStream::new(...)`, `DexIndexer::new(...)` — 동일 패턴
- [ ] `PoolDiscovery::new(provider)` → `PoolDiscovery::new(provider, network)`
- [ ] `get_pool_addresses_for_tokens(provider, tokens)` → `(provider, tokens, network)`
- [ ] `src/constants.rs`의 `get_*()` 헬퍼들 → `Network` 인자 받는 free function으로 (또는 deprecate + 새 헬퍼)
- [ ] `src/contracts/v1/dex_factory.rs`의 `WMON` flat import → `get_wmon(network)` (**Codex P1 #5 동시 해결**)

### 3. Codex Adversarial Review P1 처리

- [ ] **#1 v2 이벤트 디코더** `src/types/v2/events.rs:219`, `src/stream/v2/dex/events.rs:91`
  - `decode_log_data(log.data())` → `log.log_decode::<Event>()?` 로 변경 (indexed 필드는 topics에)
  - fixture 테스트에 실제 indexed topic 포함된 인코딩된 로그 추가
- [ ] **#3 CoreV2::create_token receipt 검증** `src/core/v2/mod.rs:201`
  - tx send 후 receipt 대기 → `Create` 이벤트 파싱 또는 `TokenRegistryV2::get_pair(prepared.token_address)` 비교
- [ ] **#4 V2CreateTokenParams 두 money 필드 검증** `src/types/v2/params.rs:248`
  - `V2CreatePayment::Native { value }`와 `buy_quote_amount` 동등성 검증 (또는 하나로 통합)
- [ ] **#6 ApiTokenInfo.version null 처리** `src/types/v1/creator.rs:104`
  - `SdkVersion`에 custom `deserialize_with` 또는 wrapper로 null → default 처리
- [ ] **#7 types/mod.rs glob export 충돌**
  - `pub use v1::*; pub use v2::*;` → explicit re-export list (alloy `sol!` 생성 `IBondingCurve` 등 internal 제외)
- [ ] **#8 clippy `-D warnings` 게이트**
  - unused imports 정리 (특히 contracts/v2/mod.rs 재익스포트 후 사용처 확인)
  - doc-comment, too-many-arguments 해결 또는 scope된 `#[allow(...)]`
  - `cargo clippy --all-targets --all-features -- -D warnings` 통과

### 4. Codex P2 처리

- [ ] **#9 estimate_gas with from=ZERO** — `Address::ZERO` 거부 또는 from 인자 분리
- [ ] **#10 V2BuyWithNativeParams.value** 인자 통합 → params 필드로 (V2CreateWithNativeParams 패턴과 일치)
- [ ] **#11 discover_pools_unified** — `TokenRegistryV2::get_pair(token)` 사용 (canonical pair) 또는 quote_token 리스트 받음
- [ ] **#12 stream global 조회** — 통합 Core 작업에서 함께 해결 (Network 인스턴스화)
- [ ] **#13 ApiClient global URL** — 통합 작업에서 함께 해결
- [ ] **#14 unified_dispatch nonce 레이스** — `Core::with_provider` 공유로 자연 해결
- [ ] **#15 prepared name/symbol** — `V2PreparedCreation`에 normalized name/symbol 포함하고 on-chain 호출에 사용

### 5. Codex P3 처리

- [ ] **#16 set_network poisoned lock** — 전역 제거 후 N/A
- [ ] **#17 VaultType::Custom** `#[serde(other)]` 추가
- [ ] **#18 NadFunSwapStream 빈 pairs** — 빈 vec 거부 또는 명시적 "all pairs" 변형 분리

### 6. 릴리스 준비

- [ ] `Cargo.toml` 버전 `0.3.12` → `0.4.0`
- [ ] `CHANGELOG.md` `[0.4.0]` 섹션 — breaking changes 명시 (`ApiClient::new(network)` 등)
- [ ] `MIGRATION.md` 신규 — 0.3.x → 0.4.0 마이그레이션 가이드 (생성자당 1줄)
- [ ] `README.md` Quick Start 업데이트 (single Core)
- [ ] `llms.txt` v2 섹션 통합 후 재작성
- [ ] `examples/` 정리 — unified Core 패턴 반영, `unified_dispatch.rs` 삭제 또는 단순화

### 7. 검증 (PR 직전)

- [ ] `cargo build --all-targets`
- [ ] `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] `cargo fmt --all --check`
- [ ] `cargo test --all-targets` — 모든 테스트 통과
- [ ] 테스트넷 라이브 스모크: `v2_smoke`, `v2_buy/sell/create_token`, `unified` 시나리오
- [ ] `/codex review` 재실행 — P1 0개, P2 잔여 항목 확인
- [ ] `branches/v2.md` Outcome 섹션 작성
- [ ] PR `v2 → mainnet`

---

## 참고

- 라이브 환경: RPC `https://dev-node.nadapp.net/` / WS `wss://dev-node.nadapp.net/` / API `https://dev-api.nadapp.net`
- 테스트넷: 97 pairs 배포됨, 거래 가능
- 메인넷 v2 주소는 `nadfun-contract-v2/.env.mainnet` 기준 — 머지 전 컨트랙트팀과 confirm 필요
- Codex 리뷰 결과는 `~/.claude/plans/nadfun-v2-next-session.md`에 상세 기록
- 14 commits: `git log main..HEAD --oneline`
