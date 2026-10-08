# ADR 013: request-bound auth proofとpolicy必須HTTP

状態: 採用済みD1/D2のSF01接続判断。日付: 2026-10-08。実装は未リリース。ユーザーがPR #100の最新RFC・判断・移行・計画を確定基準としたことを根拠とし、互換性だけを理由に旧併存へ戻さない。

## 決定

公開型・signature・ownership・Failure6分類は[SF01 contract](../security-foundation/sf01-contract.md)を正とする。`Policy[S,A]`のSは間接state protocol、Aはcallback出力protocolであり、stored proof payloadではない。Appの実shared Stateは従来のcapability検査を保つ。AuthScope/Grantはcanonical SameTaskで、Pのnominal phantomと実proofを分ける。emitterはcheckerのcanonical標準call/型/Passing/borrow_ownerとsealed planを実現する。

trusted verifierのVerifiedIdentityはfinite absolute expiryを持つassertionでproofではない。標準dispatcherのみが私有lease ownerを発行し、その期限をidentity期限へclampする。body受取後もhandlerを開始する直前に有効状態を再確認する。全security callbackの構築/pollをpanic境界へ含め、一つの絶対security budgetを共有する。

失効とnative execution permit発行は同Mutexで線形化する。予約待機はその前、enqueue callbackはgate解放後であり、permit発行=受理である。汎用Rust callbackの内部対象差し替え/deferred workを型で証明する設計は採用しない。trusted adapter契約として同期enqueueとbound対象を要求し、保証の限界を公開する。公開permit/unchecked parts/新しいlease factoryは追加しない。

旧decorator専用RoutePlan/needs_server/生成glueは、migration拒否される入口を二重保守しないため削除する。main引数禁止と旧入口元位置診断を維持する。標準routeのdynamic path登録はResult/matchitによるruntime拒否であり、文字列や関数名推測による新しいchecker routing仕様を導入しない。移行後nativeのJSON/body/path/query/Err/競合を確認する。

## 互換性と後続

Principal/旧serve/decorator/旧route arityをbreaking migration診断とする。無期限/別Task/Actor proof transfer、raw HTML、任意security header、implicit endpointを標準経路へ併存させない。標準外custom Rust hostは明示trusted境界であり、安全なstandard APIへのfallbackと説明しない。move/Task/Txの契約を維持し、SQL一本化はSF05、Session/CookieはSF02、CSRF/CORSはSF03、typed HTMLはSF04へ分ける。

## Verification / English contract

This implements adopted D1/D2 through the signatures and ownership rules in the [SF01 contract](../security-foundation/sf01-contract.md). Policy generic arguments describe protocols rather than stored proofs. Canonical SameTask traversal separates actual captures from phantom permission markers. The dispatcher alone owns a finite request lease. Invalidation and single private execution-permit issuance share one short mutex gate; reserved capacity is not admission. Arbitrary Rust adapters must synchronously enqueue the bound operation and remain trusted. Retired standard entrypoints have migration diagnostics; standard dynamic registration retains fallible runtime conflict rejection. Move, Task and Tx contracts remain unchanged. See the [public English reference](../../../docs/en/security.md) and [migration](../../../docs/en/migration-0.2.0.md) for guarantees and limits.
