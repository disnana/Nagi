# Nagi 0.3.0 candidate notes / リリース候補ノート

## 日本語

0.3.0 の候補では Rust を正式サポート対象とし、既定のバックエンドにします。Go はプレビューです。試すには互換性のある外部 Go toolchain を用意し、`nagic build` または `nagic run` に `--backend go` を指定してください。

Go プレビューでは、限定された async、ループバックで検証した HTTP、読み取り共有 State、明示的な CORS、スカラー値の単独 `json_encode` を利用できます。公開 `Task`、`scope`、`spawn`、HTTP JSON 応答、Go ネイティブアダプターは未対応です。入力形式や HTTP の制約は[対応表](../../website/content/docs/ja/reference/support.mdx)を参照してください。

Rust ランタイムの `url` は RUSTSEC-2026-0592 の修正版 2.5.9 に更新しました。Elixir の Actor／Supervisor と分散ランタイムの実験は別の作業です。

この候補は未公開です。正式公開には既存のゲートが適用され、Linux x86_64、Windows x64、macOS Intel、macOS Apple Silicon のネイティブ配布物検証が必要です。Go の配布スモークは外部 Go を使う明示オプションです。[配布と検証](../../scripts/releases/README.md)

## English

Rust is the supported and default backend for the 0.3.0 candidate. Go is a preview. To try it, provide a compatible external Go toolchain and pass `--backend go` to `nagic build` or `nagic run`.

The Go preview includes limited async, loopback-verified HTTP, shared read-only State, opt-in CORS, and standalone scalar `json_encode`. Public `Task`, `scope`, `spawn`, HTTP JSON responses, and Go native adapters remain unsupported. See the [support matrix](../../website/content/docs/en/reference/support.mdx) for input and HTTP constraints.

The Rust runtime uses `url` 2.5.9, which addresses RUSTSEC-2026-0592. Elixir Actor/Supervisor and distributed runtime experiments are separate work.

This candidate is unpublished. Existing release gates require native distribution verification on Linux x86_64, Windows x64, macOS Intel, and macOS Apple Silicon. The optional Go distribution smoke uses an external Go toolchain. [Distribution and verification](../../scripts/releases/README.md)
