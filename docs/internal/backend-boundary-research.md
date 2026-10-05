# 次フェーズで参考にした外部設計

調査日: 2026-10-05。資料の説明とNagiで検証した結果は別に扱う。過去のcompiler testing調査は[compiler-testing-research](compiler-testing-research.md)に残す。

| 一次資料・公式資料 | Nagiに関係する点 | 採用する考え | 採用しないこと |
|---|---|---|---|
| [Rust Reference: const evaluation](https://doc.rust-lang.org/reference/const_eval.html)、[operators](https://doc.rust-lang.org/reference/expressions/operator-expr.html) | const contextと通常式、整数幅、除算・overflowの違い | signed MIN除算はoverflow-checks無効でも失敗。商は0方向、余りは左辺符号。profile依存と常時失敗を分ける | rustcのMIR定数伝播・到達可能性・全lint受理集合を再実装しない |
| [Cargo external tools](https://doc.rust-lang.org/cargo/reference/external-tools.html) | compiler-messageの構造化出力 | target/src_pathとprimary/child spanを照合し、対応できる文の行へ戻す | rendered文字列からNagi列を推測しない。Rust editを自動適用しない |
| [Rust Nomicon: FFI](https://doc.rust-lang.org/nomicon/ffi.html) | ABI・unsafe・unwind境界 | 同じCargo/rustcのsource連携とC ABIを区別する。panic=abortは捕捉できない | NagiのexternをC ABIやsandboxと説明しない |
| [Axum 0.8](https://docs.rs/axum/latest/axum/)、[State](https://docs.rs/axum/latest/axum/extract/struct.State.html) | Router、extractor、Tower、型付きstate | 同じRouterとadapterでRust policy/Nagi policyを比較。transportは再実装しない | サンプルの成功を標準HTTPの全面移行や全middleware互換と扱わない |
| [Serde deserializer lifetimes](https://serde.rs/lifetimes.html) | DeserializeOwnedとborrowed Deserializeの違い | 初期版は所有DTO。borrowed JSONには入力bufferの寿命契約が必要 | DeserializeOwnedをzero-copyと説明しない。全面typed view化を始めない |
| [OWASP Authorization](https://cheatsheetseries.owasp.org/cheatsheets/Authorization_Cheat_Sheet.html) | deny-by-default、requestごとの権限確認、resource IDと対象の対応 | protected adapterがproofの対象を使い、SQLでもowner/状態を再確認。失敗経路を実HTTPで検査 | Grantの型が正しいだけでpolicy・全route・全DBの安全性を証明したと扱わない |
| [OWASP Authentication](https://cheatsheetseries.owasp.org/cheatsheets/Authentication_Cheat_Sheet.html) | OIDC/JWTのissuer・audience・署名・expiry、既存library利用 | verifierをRust資産へ委譲。Nagiの独自policyとcredential検証を分ける | 固定credentialデモをJWS実装・本番認証と扱わない。暗号を独自実装しない |

## 今回の判断への適用

普通のclassは構築・JSON復元できるDTOとして残す。opaque proofは認証結果を主張するtrusted adapterから受け取り、権限型とresourceを保護APIへ渡す。Nagi独自policyもtrusted codeで、コンパイラは業務上の正しさを証明しない。

checkerとemitterが独立に定数値やmoveを推測する構成は避ける。#74のchecked factsを維持し、型付き定数のvalidationと既存codegenを分ける。overflowの既存profile差は明示して保存する。

SQLx、SeaORM、Scylla driver、Redis/Valkeyの個別adapterは今回実装・測定していない。一般extern設計があることを、これらすべてのAPIがNagiから直接使える証拠にはしない。
