# Monad 거래 경로 최적화 조사

조사일: 2026-10-19. 이 문서는 실행·배포 제안이 아닌 공개 1차 자료와 현재 SDK 코드의 대조 결과다. 수치와 네트워크 상태는 인용한 문서 시점의 값이며, 실제 거래 경로의 절감액은 같은 호출을 같은 블록 상태에서 추정해 검증해야 한다.

## TL;DR — 우선순위

| 순위 | 항목 | 판단과 SDK 접점 |
| --- | --- | --- |
| 1 | 가스 한도 | Monad는 실제 사용량이 아니라 설정한 `gas_limit` 전체에 과금한다. V2 라우터의 **15–25%** 여유 권고를 재평가한다. [Monad Gas Pricing](https://docs.monad.xyz/developer-essentials/gas-pricing), [router.rs](../../src/contracts/v2/router.rs#L421-L430) |
| 2 | 영수증·확정성 | `latest` 및 처음 받은 영수증은 잠정적이다. 입금·정산에는 `finalized` 높이와 영수증 블록을 비교한다. [Monad JSON-RPC](https://docs.monad.xyz/reference/json-rpc/overview), [core.rs](../../src/core/core.rs#L251-L266) |
| 3 | 논스·전송 | 전송 성공 응답은 실행 성공 증거가 아니며, 논스 간격이 있어도 RPC가 처음에는 수락할 수 있다. [Monad JSON-RPC](https://docs.monad.xyz/reference/json-rpc/overview), [nonce.rs](../../src/core/nonce.rs#L208-L278) |
| 4 | 접근 목록·콜드 접근 | 메인넷 V2 거래 2건 실측에서 추정 가스 **약 51% 감소**(§3). 냉계정 **10,100 gas**, 냉스토리지 페이지 **8,100 gas**가 원인이다. 목록 자체 비용과 이미 warm인 대상을 함께 계산한다. [Opcode Pricing](https://docs.monad.xyz/developer-essentials/opcode-pricing), [EIP-2930](https://eips.ethereum.org/EIPS/eip-2930) |
| 5 | MON 잔액·수수료 | 네이티브 매수의 `value`와 최대 가스비를 함께 예산화한다. **10 MON** 예비 잔액 규칙에는 조건부 예외가 있다. [Reserve Balance](https://docs.monad.xyz/developer-essentials/reserve-balance), [dex.rs](../../src/contracts/v1/dex.rs#L40-L68) |
| 6 | 수수료 선택 | 기본 수수료 하한은 **100 MON-gwei**이며 우선순위 수수료는 혼잡도에 따라 조정해야 한다. [Gas Pricing](https://docs.monad.xyz/developer-essentials/gas-pricing), [trade.rs](../../src/types/v1/trade.rs#L18-L35) |
| 7 | 스트림·RPC | `monadNewHeads`/`monadLogs`는 확정 상태를 전달한다. 현재 SDK의 표준 `logs` 구독과 의미가 다르다. [Monad JSON-RPC](https://docs.monad.xyz/reference/json-rpc/overview), [stream.rs](../../src/stream/v2/curve/stream.rs#L58-L70) |
| 8 | EIP-7702 | 위임 계정의 배치 실행은 가능하지만 SDK가 자동으로 승인과 거래를 묶지는 않는다. 위임 계정의 예비 잔액 제한도 적용된다. [Monad EIP-7702](https://docs.monad.xyz/developer-essentials/eip-7702) |
| 9 | 블록 시간·파이프라인 | 짧은 블록 간격은 폴링·재시도 정책의 입력이지 즉시 확정 보증이 아니다. [MIP-12](https://github.com/monad-crypto/MIPs/blob/main/MIPs/MIP-12.md), [Monad JSON-RPC](https://docs.monad.xyz/reference/json-rpc/overview) |
| 10 | 다음 하드포크 | Amsterdam/EIP-7981 구현과 현재 네트워크 활성화는 구분해야 한다. 선행 최적화의 장기 가정으로만 취급한다. [monad-bft v0.16.3](https://github.com/category-labs/monad-bft/releases/tag/v0.16.3), [EIP-7981](https://eips.ethereum.org/EIPS/eip-7981) |

## 1. `charges_gas_limit`와 가스 한도

Monad 공식 문서는 `gas_paid = gas_limit × price_per_gas`이며 실행 중 사용한 가스가 적어도 제출한 한도 전체에 과금한다고 명시한다. 실행 소스의 `charges_gas_limit()`은 `MONAD_ONE` 이상에서 참이며 미사용 가스 환급은 0을 반환한다. 따라서 추정치에 무조건 여유를 곱하면 한도 증가분을 실제로 지불한다. [Gas Pricing](https://docs.monad.xyz/developer-essentials/gas-pricing), [VM traits](https://github.com/category-labs/monad/blob/main/category/vm/evm/traits.hpp), [transaction gas](https://github.com/category-labs/monad/blob/main/category/execution/monad/monad_transaction_gas.cpp)

SDK의 [V1 가스 추정](../../src/core/v1/gas.rs#L58-L141), [V1 전송 옵션](../../src/contracts/v1/bonding_curve.rs#L61-L103)과 [V2 라우터 추정](../../src/contracts/v2/router.rs#L421-L617)을 먼저 대조할 지점으로 본다. 특히 V2 rustdoc의 **15–25%** 추가 한도 권고는 Monad에서는 가격과 직접 연결된다. 이 비율을 임의로 줄이는 것도 OOG 위험을 높이므로, 호출별 추정치·실제 사용량·실패율을 모아 상한 정책을 정하는 것이 타당하다. 가스 사용량이 고정인 동작은 명시적 한도를 권장한다는 공식 문서의 범위만 확정적으로 말할 수 있다. [Gas Pricing](https://docs.monad.xyz/developer-essentials/gas-pricing), [router.rs](../../src/contracts/v2/router.rs#L421-L430)

**적용 판단 — P0.** 기존 버퍼 안내를 바로잡고 거래 경로별 한도 정책을 측정하면 과금 낭비를 줄일 수 있다. 문서 수정만으로는 공개 API 변경이 없지만, 기본 한도 산정 동작을 바꾸면 버전 결정과 changelog가 필요하다. [router.rs](../../src/contracts/v2/router.rs#L421-L430), [Gas Pricing](https://docs.monad.xyz/developer-essentials/gas-pricing)

## 2. 냉계정·스토리지 페이지·MIP-8

공식 요금표는 냉계정 접근 **10,100 gas**, 냉스토리지 접근 **128개 연속 슬롯의 페이지당 8,100 gas**, warm 계정·스토리지 접근 **100 gas**를 제시한다. 페이지는 `slot >> 7`로 계산하며 동일 `(account, page)` 안의 뒤따르는 접근은 같은 트랜잭션 동안 warm이다. 이는 슬롯별 냉비용을 단순 합산하면 과대평가될 수 있음을 뜻한다. [Opcode Pricing](https://docs.monad.xyz/developer-essentials/opcode-pricing), [MIP-8](https://github.com/monad-crypto/MIPs/blob/main/MIPs/MIP-8.md)

SDK의 [V1 매수 가스 추정](../../src/core/v1/gas.rs#L168-L204), [매도 추정](../../src/core/v1/gas.rs#L235-L270), [V2 라우터 추정](../../src/contracts/v2/router.rs#L421-L617)에 영향이 있을 수 있다. 실제 페이지 분포는 계약 저장 레이아웃과 실행 경로에 달려 있으므로 SDK만 보고 절감량을 확정할 수 없다. 공식 문서는 MIP-8 규칙이 `MONAD_TEN`에서 활성화된다고 적고, 현재 공개 메인넷 체인 설정은 **2026-09-02 14:30 UTC**부터 `MONAD_TEN`을 선택한다. [Opcode Pricing](https://docs.monad.xyz/developer-essentials/opcode-pricing), [mainnet config](https://github.com/category-labs/monad/blob/main/category/execution/monad/chain/monad_mainnet.cpp)

**적용 판단 — P1.** 페이지 단위 접근을 반영한 가스 추정 비교가 과대한 한도를 찾는 데 유용하다. 현 추정 메서드 내부의 분석만으로는 공개 API 변경이 없으며, 반환 의미나 기본 정책을 바꾸면 버전 결정을 검토해야 한다. [gas.rs](../../src/core/v1/gas.rs#L168-L204), [Opcode Pricing](https://docs.monad.xyz/developer-essentials/opcode-pricing)

## 3. EIP-2930 접근 목록

EIP-2930은 목록에 계정당 **2,400 gas**, 키당 **1,900 gas**의 내재비용을 붙인다. Monad의 MIP-8에서는 목록에 넣은 저장 키 하나가 해당 **128슬롯 페이지**를 warm으로 만들며 `eth_createAccessList`는 키를 페이지별로 중복 제거한다. 단 한 번 냉접근을 피하는 예시의 순절감은 계정 `10,100 − 100 − 2,400 = 7,600 gas`, 페이지 `8,100 − 100 − 1,900 = 6,100 gas`이지만, 실제 결과는 접근 여부·중복·뒤따르는 사용에 달려 있다. [EIP-2930](https://eips.ethereum.org/EIPS/eip-2930), [Opcode Pricing](https://docs.monad.xyz/developer-essentials/opcode-pricing), [JSON-RPC](https://docs.monad.xyz/reference/json-rpc/overview)

송신자, 수신자, 프리컴파일은 EIP-2929에서 실행 시작 시 warm이고, COINBASE는 EIP-3651에서 warm이 되었다. 이미 warm인 주소를 목록에 넣으면 목록 비용만 늘 수 있으므로 후보에서 제외해야 한다. 공개 Monad 메인넷 설정의 `MONAD_TEN`은 실행 소스에서 Osaka EVM으로 매핑되므로 COINBASE도 warm 대상으로 취급한다. [EIP-2929](https://eips.ethereum.org/EIPS/eip-2929), [EIP-3651](https://eips.ethereum.org/EIPS/eip-3651), [VM traits](https://github.com/category-labs/monad/blob/main/category/vm/evm/traits.hpp), [mainnet config](https://github.com/category-labs/monad/blob/main/category/execution/monad/chain/monad_mainnet.cpp)

SDK는 [공통 V2 트랜잭션 옵션](../../src/contracts/v2/router.rs#L27-L54)과 [V1 전송 옵션](../../src/contracts/v1/bonding_curve.rs#L61-L103)을 통해 목록 추가 가능성을 검토할 수 있다. `eth_createAccessList` 결과를 거래별로 채택할지는 목록 유무의 동일 조건 추정값을 비교한 뒤 판단해야 한다. EIP-2930 목록은 트랜잭션 유형 **1**뿐 아니라 EIP-1559 유형 **2**의 필드에도 포함될 수 있다. [Monad Transactions](https://docs.monad.xyz/developer-essentials/transactions), [EIP-1559](https://eips.ethereum.org/EIPS/eip-1559)

**실측 (2026-10-19, `https://rpc-mainnet.monadinfra.com`, 트랜잭션 직전 블록 상태 기준).** 메인넷 V2 라우터 트랜잭션 2건에 대해 `eth_createAccessList` 결과(주소 16개, 키 27개)를 넣고 `eth_estimateGas`를 비교했다. `sell` [0x3a35…0035](https://monadvision.com/tx/0x3a355c89eda10f82f4f4f4e22db0ac707e8765762815aca507d010aea1920035)은 **552,896 → 263,088 (−52%)**, `exactOutBuy` [0xca81…7afa](https://monadvision.com/tx/0xca813d32df0ac52e6fecfed3ae712907cdc97399331e8b342c0d2057b9857afa)은 **560,209 → 274,963 (−51%)**이었다. 두 트랜잭션은 실제로 gas limit **1,000,000**으로 제출되어 그만큼 과금되었다. 표본 2건, V2 ERC-20 경로만이며 native·V1 경로는 미측정이다.

**0.6.0 SDK 경로 검증.** 메인넷(읽기 전용): `Core::new` 서명 provider의 `estimate_gas`(리스트 자동)와 root provider(리스트 없음)를 직전 블록 상태에서 비교했다. V2 `sell`/`exactOutBuy` 10건과 합성 `buyWithNative` 1건, 총 11건 모두 **50.8–52.4%** 감소했다. 테스트넷(실제 전송, chain 10143): curve 단계 토큰 `0xc909…7777`에 V2 `buy` 1 USDC를 리스트 없음/자동으로 3쌍 전송했다. 6건 모두 성공했고 모두 type 2였다. receipt `gasUsed`(= gas limit, Monad 과금 기준)는 리스트 없음 **546,986**(첫 건 581,525), 자동 **260,114**로 **−52.4%**였다. 자동 쪽 리스트는 주소 13개, 키 31개였다. 자동 쪽 tx: [0x01af…79a6](https://testnet.monadvision.com/tx/0x01afb541749f8523ba30839be3997e87360a6f8b3a043425ff7b80cbcf3f79a6), [0x4cf1…ef13](https://testnet.monadvision.com/tx/0x4cf121d4091c36add43b01f496ce858de577e45b5f7969882d56b7200e45ef13), [0x9794…584e](https://testnet.monadvision.com/tx/0x9794d80ebd14718748c1c7e2254de770f1fb995bb5bc2d8c6c13562826ba584e). 리스트 없음 쪽 tx: [0x9f8f…69db](https://testnet.monadvision.com/tx/0x9f8f45951abe8350434f95b79b8c67d76fb400ba59f9545db5ce99bfe19969db). MON(WMON) quote 토큰 `0x3fd7…7777`에는 native 경로 `buyWithNative` 0.01 MON을 같은 방식으로 3쌍 전송했다. 6건 모두 성공했고 모두 type 2였다. 리스트 없음 **554,292**(첫 건 571,562), 자동 **265,897**로 **−52.0%**였다(주소 14개, 키 30개). 자동 쪽 tx: [0x1ec6…ed12](https://testnet.monadvision.com/tx/0x1ec6c5bb8cfbd63b5936a506b8d8c0d7d917c74a0bbce58884c0c3b1b272ed12), [0xe445…bea4](https://testnet.monadvision.com/tx/0xe445fdc218db933c2f383ab2bee2de4b81088f0b8fc8b586bd63ae2eef4abea4), [0x74bf…f493](https://testnet.monadvision.com/tx/0x74bf684cff4077a3c4cd57ae87f5f2b0545c0c15c7fa95ffe1c95cd75e01f493).

**적용 판단 — P0 (실측 반영).** 거래별 순절감이 확인된 경우에만 목록을 붙이면 가스비를 줄일 가능성이 있다. 공개 매개변수 구조체에 필드를 추가하는 방안은 기존 구조체 리터럴에 영향을 주므로 공개 API 변경과 버전 결정이 필요하다. 내부 선택적 전송만으로 끝내는 방안은 공개 시그니처를 유지할 수 있다. [router.rs](../../src/contracts/v2/router.rs#L27-L54), [EIP-2930](https://eips.ethereum.org/EIPS/eip-2930)

## 4. EIP-7702와 승인·거래 배치

Monad는 EIP-7702 거래 유형 **4**를 지원한다. EOA를 코드에 위임하면 배치 실행이 가능하지만, 승인과 거래가 한 트랜잭션에서 원자적으로 처리되려면 선택한 위임 코드가 그 호출과 권한 검사를 실제로 구현해야 한다. 현재 SDK의 [토큰 승인](../../src/token/token.rs#L133-L136)과 [V2 라우터 전송](../../src/contracts/v2/router.rs#L66-L320)이 자동으로 같은 트랜잭션으로 합쳐진다는 근거는 찾지 못했다. [Monad EIP-7702](https://docs.monad.xyz/developer-essentials/eip-7702), [EIP-7702](https://eips.ethereum.org/EIPS/eip-7702)

위임 EOA에서 잔액을 **10 MON** 아래로 낮추는 거래는 되돌려질 수 있고, 위임된 코드의 `CREATE`/`CREATE2` 호출은 금지된다. 따라서 배치의 지연·수수료 이득은 권한·서명·잔액 및 계약 호환성 검토 후 측정해야 한다. [Monad EIP-7702](https://docs.monad.xyz/developer-essentials/eip-7702)

**적용 판단 — P2.** 호환 위임 코드가 있을 때 승인과 거래의 왕복 전송을 줄일 수 있지만 자동 절감액은 확인되지 않았다. 배치 메서드나 위임 설정을 SDK에 공개하면 새 공개 API이므로 버전 결정을 거친다. [router.rs](../../src/contracts/v2/router.rs#L66-L320), [EIP-7702](https://eips.ethereum.org/EIPS/eip-7702)

## 5. 네이티브 MON 거래의 예비 잔액

예비 잔액 매개변수는 **10 MON**이며 현재 지연 실행의 `k`는 **3블록**이라고 공식 문서는 설명한다. 가스 지출은 `gas_price × gas_limit`, 네이티브 `value`는 별도 지출이다. 미위임 EOA의 특정 첫 거래에는 잔액을 예비선 아래로 비우는 예외가 있으므로 모든 네이티브 매수에 일괄적인 “10 MON 유지” 검사를 두면 정상 거래를 막을 수 있다. 위임 EOA에는 이 예외가 없다. [Reserve Balance](https://docs.monad.xyz/developer-essentials/reserve-balance)

[V1 DEX 매수](../../src/contracts/v1/dex.rs#L40-L68), [V1 토큰 생성](../../src/contracts/v1/bonding_curve.rs#L76-L104), [V2 라우터 전송](../../src/contracts/v2/router.rs#L66-L320)은 `value`, 가스 한도, 최대 수수료를 함께 고려해야 하는 경로다. 잔액 부족 가치 지출 거래가 블록에 포함된 뒤 되돌려지고도 가스를 낼 수 있으므로 사전 견적은 사용자 비용 예방에 유용하지만 실행 성공 보증은 아니다. [Reserve Balance](https://docs.monad.xyz/developer-essentials/reserve-balance)

**적용 판단 — P0.** 잔액과 거래 비용을 사전에 대조하면 일부 유료 실패를 예방할 수 있다. 기존 전송에서 새 사전 거절을 기본 적용하면 런타임 동작 변경이므로 버전 결정이 필요하며, 예외 조건을 빠뜨린 일괄 거절은 피해야 한다. [dex.rs](../../src/contracts/v1/dex.rs#L40-L68), [Reserve Balance](https://docs.monad.xyz/developer-essentials/reserve-balance)

## 6. 기본·우선순위 수수료

Monad의 EIP-1559 지불 가격은 `min(base_fee + priority_fee, max_fee)`이며 문서의 최소 기본 수수료는 **100 MON-gwei**이다. 기본 클라이언트는 총 가스 가격이 높은 순으로 거래를 정렬한다. `eth_maxPriorityFeePerGas`가 현재 반환하는 **2 gwei**는 임시 고정 제안값이라 실시간 혼잡도 측정값으로 해석하면 안 된다. `eth_feeHistory`의 `latest` 뒤에 붙는 예상 기본 수수료 역시 실제 예측이 아닌 마지막 값의 반복이다. [Gas Pricing](https://docs.monad.xyz/developer-essentials/gas-pricing), [JSON-RPC](https://docs.monad.xyz/reference/json-rpc/overview)

SDK의 [GasPricing 기본값](../../src/types/v1/trade.rs#L18-L35)은 `Legacy`이고 EIP-1559 선택지가 있다. 그러나 [V1](../../src/contracts/v1/bonding_curve.rs#L82-L96)과 [V2 공통 옵션](../../src/contracts/v2/router.rs#L33-L48)에서 `Legacy`는 가격 필드를 설정하지 않는 분기이며, SDK가 만든 서명 공급자는 [Alloy gas estimation filler](../../src/core/nonce.rs#L318-L327)를 사용한다. 따라서 `Legacy`라는 이름만으로 최종 거래가 유형 0 또는 고정 가격이라고 단정하면 안 된다. 현재 권고는 공급자의 동적 추정을 기본으로 유지하고, 호출자가 필요할 때 명시적 EIP-1559 상한과 tip을 선택하도록 하는 것이다. 체인 문서만으로 안전한 고정 tip 값을 추천할 수 없다. [Gas Pricing](https://docs.monad.xyz/developer-essentials/gas-pricing), [JSON-RPC](https://docs.monad.xyz/reference/json-rpc/overview)

**적용 판단 — P1.** 현재 기본값을 무조건 고정 tip으로 대체하기보다, 수수료 상한과 포함 지연의 관계를 측정한 뒤 선택 정책을 정하면 과금·대기 시간의 균형을 조정할 수 있다. `GasPricing::default()` 또는 자동 fee filler 동작 변경은 공개 동작 변경으로 버전 결정이 필요하다. [trade.rs](../../src/types/v1/trade.rs#L18-L35), [Gas Pricing](https://docs.monad.xyz/developer-essentials/gas-pricing)

## 7. 논스·로컬 멤풀·파이프라인

Monad에는 전역 멤풀이 없고 RPC 노드는 예정된 리더들의 로컬 멤풀에 거래를 전달한다. `eth_sendRawTransaction`은 논스 간격이나 가스 잔액 부족을 즉시 거절하지 않을 수 있으며, 보류 거래는 `eth_getTransactionByHash`에 나오지 않는다. 그러므로 해시 반환을 포함·실행 증거로 사용하거나, 보류 조회의 `null`을 거래 유실 증거로 단정할 수 없다. [Local Mempool](https://docs.monad.xyz/monad-arch/consensus/local-mempool), [JSON-RPC](https://docs.monad.xyz/reference/json-rpc/overview)

SDK의 [논스 관리자](../../src/core/nonce.rs#L1-L54)는 계정별 전송을 조정하지만 [체인 논스 조회](../../src/core/nonce.rs#L343-L345)는 `latest`를 사용한다. 빠른 연속 전송은 순서·간격·재전송·교체 정책과 결합되어야 한다. 공식 자료에서 안전한 최대 파이프라인 깊이나 고정 재시도 간격은 찾지 못했다. [Pipelining](https://docs.monad.xyz/monad-arch/concepts/pipelining), [Local Mempool](https://docs.monad.xyz/monad-arch/consensus/local-mempool)

**적용 판단 — P1.** [자동 전송 경로](../../src/core/nonce.rs#L237-L278)의 계정 잠금과 전송 전 `latest` 조회가 지연 요인이다. 계측 후 제한된 파이프라인을 검토할 수 있지만, 허용 논스 간격·모호한 전송 결과의 복구까지 설계해야 한다. 기존 자동 논스 동작을 바꾸면 공개 동작 변경으로 버전 결정이 필요하다. [nonce.rs](../../src/core/nonce.rs#L237-L278), [JSON-RPC](https://docs.monad.xyz/reference/json-rpc/overview)

## 8. 블록 시간·확정 상태·영수증

MIP-12는 투표 간격을 **400 ms에서 300 ms**로 줄이고 블록 한도도 비례 조정하는 규칙을 명시한다. RPC의 `latest`는 `Proposed`, `safe`는 `Voted`, `finalized`는 `Finalized`를 가리킨다. 미확정 블록에서 얻은 영수증의 블록 번호·로그·응답 자체가 바뀔 수 있다. 금전적 정산은 영수증 `blockNumber`가 `finalized` 높이 이하인지 확인하고 `status`도 검증해야 한다. [MIP-12](https://github.com/monad-crypto/MIPs/blob/main/MIPs/MIP-12.md), [JSON-RPC](https://docs.monad.xyz/reference/json-rpc/overview)

SDK의 [공개 `get_receipt`](../../src/core/core.rs#L251-L266)은 즉시 한 번 조회하고, 내부 [영수증 대기](../../src/core/core.rs#L351-L378)는 **500 ms** 간격으로 폴링한다. 이 값은 SDK 코드의 현 상태이며, **300 ms** 블록 간격에 맞춘 최적값이라는 근거는 아니다. [MIP-12](https://github.com/monad-crypto/MIPs/blob/main/MIPs/MIP-12.md), [core.rs](../../src/core/core.rs#L351-L378)

**적용 판단 — P0.** 영수증 존재·`status`·확정 높이를 목적에 따라 구분하면 조기 성공 판정을 막을 수 있다. 공개 `wait_for_receipt`/확정 대기 메서드를 더하는 경우 새 공개 API와 버전 결정이 필요하고, 기존 `get_receipt`의 의미를 바꾸는 경우에는 호환성 영향이 더 크다. [core.rs](../../src/core/core.rs#L251-L266), [JSON-RPC](https://docs.monad.xyz/reference/json-rpc/overview)

## 9. 동기 전송 RPC와 Monad 상태 스트림

공식 JSON-RPC 목록은 `eth_sendRawTransactionSync`를 지원하며, 서명된 거래와 `timeout_ms`를 받아 영수증 형태의 결과를 반환한다고 기술한다. 이 메서드의 반환도 미확정 블록에서 나올 수 있으므로 최종성 검사를 대체하지 않는다. SDK [논스 관리자](../../src/core/nonce.rs#L388-L426)는 동기 전송 훅을 구현하지만, 현재 라우터 전송 메서드는 `send()`를 호출한다. [JSON-RPC Overview](https://docs.monad.xyz/reference/json-rpc/overview), [JSON-RPC API](https://docs.monad.xyz/reference/json-rpc/api#eth_sendrawtransactionsync), [router.rs](../../src/contracts/v2/router.rs#L89-L92)

문서에 나오는 Monad 고유 RPC는 `monad*`로 시작하는 HTTP 메서드만이 아니라 `txpool_statusByAddress`/`txpool_statusByHash`와 구독 유형 `monadNewHeads`/`monadLogs`를 포함한다. 따라서 이름만으로 범용 `monad_*` 메서드 집합을 가정하면 안 된다. `monadNewHeads`와 `monadLogs`는 `blockId`, `commitState`를 추가하고 `Proposed`→`Voted`→`Finalized`→`Verified` 진행을 알린다. SDK의 [V1 로그](../../src/stream/v1/curve/stream.rs#L81-L81), [V2 곡선 로그](../../src/stream/v2/curve/stream.rs#L58-L70), [V2 DEX 로그](../../src/stream/v2/dex/stream.rs#L138-L147)는 표준 `subscribe_logs` 기반이므로 확정 상태가 필요하면 별도 처리 경로가 필요하다. [JSON-RPC](https://docs.monad.xyz/reference/json-rpc/overview)

**적용 판단 — P1.** 동기 전송은 별도 영수증 폴링 호출을 줄일 수 있고, 상태 구독은 확정 시점을 더 빨리 알려줄 수 있다. 새 전송·구독 메서드를 공개한다면 공개 API 추가 및 버전 결정이 필요하다. 현재 표준 구독의 이벤트 의미를 바꾸는 선택은 기존 호출자와의 호환성 검토가 필요하다. [nonce.rs](../../src/core/nonce.rs#L388-L426), [JSON-RPC](https://docs.monad.xyz/reference/json-rpc/overview)

## 10. `MONAD_NEXT`·Amsterdam·EIP-7981·MIP-12

Category Labs의 `monad-bft` **v0.16.3** 릴리스 노트는 EIP-7981 구현이 `eip_7981_active` 뒤에 있고 `MONAD_ETH_AMSTERDAM` 이상에서만 활성화되며 당시 네트워크에서는 접근 불가라고 명시한다. 실행 코드의 revision 매핑은 `MONAD_NEXT` 이상을 Amsterdam으로 매핑하지만, 공개 메인넷 설정의 최신 규칙은 `MONAD_TEN`이다. [monad-bft v0.16.3](https://github.com/category-labs/monad-bft/releases/tag/v0.16.3), [VM traits](https://github.com/category-labs/monad/blob/main/category/vm/evm/traits.hpp), [revision.h](https://github.com/category-labs/monad/blob/main/category/vm/evm/monad/revision.h), [mainnet config](https://github.com/category-labs/monad/blob/main/category/execution/monad/chain/monad_mainnet.cpp)

EIP-7981 제안 본문은 접근 목록 데이터에 주소당 **1,280 gas**, 키당 **2,048 gas**의 추가 비용을 제안한다. 다만 Monad 실행 소스의 미래 경로는 주소 **20바이트**, 키 **32바이트**에 **바이트당 40 gas**를 적용하므로 제안 본문의 **64 gas/byte**와 다르다. 현재 네트워크 과금으로 어느 쪽도 인용해서는 안 된다. 미래 규칙이 활성화되면 EIP-2930의 기존 **2,400 gas/주소**, **1,900 gas/키** 비용 외에 데이터 비용이 생겨 접근 목록의 순절감이 달라진다. [EIP-7981](https://eips.ethereum.org/EIPS/eip-7981), [Monad transaction_gas.cpp](https://github.com/category-labs/monad/blob/main/category/execution/ethereum/transaction_gas.cpp), [EIP-2930](https://eips.ethereum.org/EIPS/eip-2930)

MIP-12는 블록 시간·한도 변경이며 접근 목록 재가격 책정의 근거 문서가 아니다. 소스에서 `MONAD_NEXT`→Amsterdam 매핑은 확인되지만, 메인넷 `MONAD_NEXT` 활성 시간은 확인되지 않았다. 따라서 SDK의 가스·접근 목록 정책은 현재 활성 규칙을 기준으로 하고 하드포크 시점에 다시 검증해야 한다. [MIP-12](https://github.com/monad-crypto/MIPs/blob/main/MIPs/MIP-12.md), [VM traits](https://github.com/category-labs/monad/blob/main/category/vm/evm/traits.hpp), [mainnet config](https://github.com/category-labs/monad/blob/main/category/execution/monad/chain/monad_mainnet.cpp)

**적용 판단 — P2.** 현재 SDK에 선반영할 과금 변경은 확인되지 않았다. 포크 활성화 전후에 접근 목록의 순절감과 가스 한도를 다시 측정하는 운영 항목이며, 조사 자체는 공개 API 변경이 없다. 향후 자동 정책을 변경하면 별도 버전 결정을 거친다. [VM traits](https://github.com/category-labs/monad/blob/main/category/vm/evm/traits.hpp), [router.rs](../../src/contracts/v2/router.rs#L27-L54)

## 미확인 사항과 검증 질문

- `charges_gas_limit()` 구현과 현재 메인넷 `MONAD_TEN` 선택은 확인했다. 향후 포크에서 과금 규칙이 바뀔지는 네트워크별 활성 공지와 코드로 다시 확인해야 한다. [VM traits](https://github.com/category-labs/monad/blob/main/category/vm/evm/traits.hpp), [mainnet config](https://github.com/category-labs/monad/blob/main/category/execution/monad/chain/monad_mainnet.cpp)
- 접근 목록의 실제 SDK 거래별 절감액과 `eth_createAccessList` 공급자 간 결과 차이는 재현 가능한 동일 상태 추정으로 확인해야 한다. [Opcode Pricing](https://docs.monad.xyz/developer-essentials/opcode-pricing), [JSON-RPC](https://docs.monad.xyz/reference/json-rpc/overview)
- `MONAD_NEXT`→Amsterdam 매핑은 코드에서 확인했지만 EIP-7981의 메인넷 활성 시간은 확인되지 않았다. 구현 존재를 현재 활성화로 간주하지 않는다. 또한 EIP 본문의 **64 gas/byte**와 Monad 구현의 **40 gas/byte** 차이는 활성 전에 확인해야 한다. [VM traits](https://github.com/category-labs/monad/blob/main/category/vm/evm/traits.hpp), [EIP-7981](https://eips.ethereum.org/EIPS/eip-7981), [Monad transaction_gas.cpp](https://github.com/category-labs/monad/blob/main/category/execution/ethereum/transaction_gas.cpp)
- 공식 Gas Pricing의 요약은 블록 한도 **150M gas**를 말하지만 같은 페이지의 수식 예시는 목표 **160M gas (80%)**라고 써 내부 불일치가 있다. 블록 한도 계산은 MIP-12 및 운영 네트워크 설정을 별도로 확인해야 한다. [Gas Pricing](https://docs.monad.xyz/developer-essentials/gas-pricing), [MIP-12](https://github.com/monad-crypto/MIPs/blob/main/MIPs/MIP-12.md)
- 현 저장소의 [Cargo.toml](../../Cargo.toml#L1-L40)은 SDK **0.5.0**, Alloy **2.5.0**으로 확인되며 루트 `AGENTS.md`의 버전 서술과 다르다. 버전 사실은 실제 manifest를 우선한다.

## Sources

- Monad 공식 문서: [Gas Pricing](https://docs.monad.xyz/developer-essentials/gas-pricing), [Opcode Pricing](https://docs.monad.xyz/developer-essentials/opcode-pricing), [Transactions](https://docs.monad.xyz/developer-essentials/transactions), [Differences](https://docs.monad.xyz/developer-essentials/differences), [Reserve Balance](https://docs.monad.xyz/developer-essentials/reserve-balance), [EIP-7702](https://docs.monad.xyz/developer-essentials/eip-7702), [JSON-RPC Overview](https://docs.monad.xyz/reference/json-rpc/overview), [JSON-RPC API](https://docs.monad.xyz/reference/json-rpc/api), [Local Mempool](https://docs.monad.xyz/monad-arch/consensus/local-mempool), [Pipelining](https://docs.monad.xyz/monad-arch/concepts/pipelining).
- 프로토콜 제안: [MIP-8](https://github.com/monad-crypto/MIPs/blob/main/MIPs/MIP-8.md), [MIP-12](https://github.com/monad-crypto/MIPs/blob/main/MIPs/MIP-12.md), [EIP-2929](https://eips.ethereum.org/EIPS/eip-2929), [EIP-2930](https://eips.ethereum.org/EIPS/eip-2930), [EIP-3651](https://eips.ethereum.org/EIPS/eip-3651), [EIP-7702](https://eips.ethereum.org/EIPS/eip-7702), [EIP-7981](https://eips.ethereum.org/EIPS/eip-7981).
- 구현·릴리스: [Category Labs monad execution](https://github.com/category-labs/monad), [monad-bft v0.16.3](https://github.com/category-labs/monad-bft/releases/tag/v0.16.3). 현재 SDK 위치는 각 절의 상대 링크에 표시했다.
