# ADR 003: Nagi位置とRust診断詳細

状態: 採用。位置精度を誇張せず、既存raw診断を取得可能に保つ。

現行GeneratedはRust/Lowの行から元の文・定義の行へ対応する。Source identityはcanonical path/module ID、sourceのSpanはfile-local token rangeである。High→Lowで復元するのは行であり、元Highの式token範囲は持ち越さない。

今回もNagiのファイル・行と対応する関連cause noteを示す。列、Rustの修正edit、未対応primary、dependency/native/synthetic位置を推測でNagiへ変換しない。source mappingはstatement-line精度であり、expression-span精度ではない。

CLIの通常表示ではmapped primaryと関連causeを優先し、巨大な生成Rust本文を重ねない。`build/run --rust-diagnostics`でraw Rust診断を取得できるようにする。元Rustへしか対応しないdiagnosticはrawのまま保ち、エラーを黙って削除しない。既存ライブラリ向けcargo_message APIの詳細表示は維持する。

将来のexpression mappingには、High/Low共通source identityとstable expression IDをchecked representationまで運ぶ必要がある。列を行から推測する案、Rustの`.clone()`提案をNagiへ自動適用する案は不採用。
