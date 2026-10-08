# ADR 001: Rust資産と認証・認可の境界

> 2026-10-08更新: 以下のPrincipal/無期限issuerは公開済み0.1.11の実験記録です。未リリース0.2.0 SF01の現行契約は[ADR 013](013-request-bound-auth-and-http-policy.md)で置換します。古い実験を現行標準APIや互換fallbackとして使用しません。

状態: 次フェーズの最小実験として採用。標準APIの安定性や全アプリの安全性を約束する決定ではない。

## 目的

NagiはRust/Goの置換を目指さない。Rustの性能とライブラリを利用し、HTTP、認証、認可、validation、DB、データ処理の危険な境界を安全かつ簡潔に記述する。Rust/Cargoによるnative生成を続ける。

## Rust interop

現行の`@rust`・typed extern・Cargo依存を使う。同一Rustビルドのsource連携で、C ABIではない。引数・returnのNagi型、move/view/shared、Option/Result、sync/asyncを明示する。crate API・trait・最終Send/Syncの整合はrustcで確認する。unsafe契約、credential検証、policyの正しさはアダプター作者と検証ライブラリの責任で、テストとレビューで確認する。rustcが認証・認可の判断を証明するわけではない。

通常のNagiにRust lifetimeを書かせない。借用元を証明できなければ拒否し、cloneやstatic化で回避しない。externのview returnは現行の制限を維持する。任意generic/trait object/unsafe/raw pointer APIはadapterで対応型へ変換する。safe Rust関数の署名だけで、その内部にunsafeやpanicがないことを証明したとはしない。

## 最小Auth契約

通常classは型付きDTOに使う。構築・JSON復元できるclassを認証済み/認可済みの証明には使わない。

最小実験はopaque `Principal` と消費型 `Grant[P]`。`P`はclass/enumによるnominalな権限marker。Nagiからproofを構築・JSON復元・暗黙cloneできない。Rust側のフィールドもprivateにする。proofを共有・class fieldへ保存する仕組みは初回に追加しない。

信頼したRust adapterがcredentialを検証してPrincipalを返す。subject/resource/actionに対するpolicyはNagiにも書ける。デモではRust authorizerが名前付きNagi async policyをawaitし、許可された場合だけGrantを返す。保護operationはそのGrantをmoveで消費し、resource IDはGrantから得る。別のbare IDを指定して許可対象をすり替えない。Grant[Read]とGrant[Write]は異なる型である。

JWSの署名・algorithm・issuer・audience・期限・鍵管理は既存Rustライブラリを使うverifierの責任とする。初回デモは固定credentialで境界を検査し、JWS verifier自体は実装しない。verifierを差し替える入口と、Nagiで独自policyを追加する入口を分ける。普通のclassによるclaims・入力・業務errorの型付けは引き続き有効だが、claimsを構築できることを検証成功と扱わない。

policyが常に許可する実装や、adapterがpolicyを呼ばずmintする実装をcheckerは見抜けない。Nagi policyも信頼するアプリコードであり、その内容はテストとレビューで確認する。bool一つを渡せば認可済みと称する標準factoryは公開しない。

「認可漏れ」は、この実験では**保護APIが要求する権限型のGrantを渡さないこと**と定義する。認証済みPrincipalを要求するauthorizerへPrincipalを渡さないこともcheckで拒否する。関数名、bool、route文字列、SQL文字列から認証・認可を推測しない。

externは既存の信頼境界である。proofを返すextern署名はRust issuerを信頼する明示宣言であり、実装のcredential/policy検証をcheckが証明するわけではない。虚偽issuer、保護APIを迂回するRust/DB access、改変されたライブラリは保証範囲外。宣言を書くだけのmanifest安全判定は採らない。

## 保証しないもの

全routeへの認証必須、DTOのresponse流出、全SQLのtenant制約、同request内限定、撤回・期限・TOCTOU・外部副作用の静的証明は行わない。public routeはRust/Axum例で明示するが、既存routeを自動でprotectedへ変更しない。

owned proofをasync処理へmoveすることは明示的delegationとして扱う。background/service-to-service用のcredential、期限、再認可、audienceはtrusted adapterの契約で決める。request終了だけで所有proofが無効になるとは説明しない。panic/取消はrollbackではない。

## 不採用案

- 普通のPrincipal/Grant class: DTOとして有効だが、proofは手作業やJSONから偽造できる。
- route名・SQL名によるheuristic: 意図と実行条件を区別できない。
- 一般taint/effect/任意crate解析: 現在のIRと規模に対して保証・費用・移行の根拠が不足。
- 独自認証/暗号/DB driver: Rust資産を使う目的に反する。proof wrapperはpolicy engineを再実装しない。

## 検証

missing/fake/wrong-permission/JSON/shared/nested wrapperのnegative、成功・401・403・対象すり替え・malformed入力の実HTTP、High/保存Low/手書きLowを確認する。型checkの成功と実際のpolicy成功を別に記録する。
