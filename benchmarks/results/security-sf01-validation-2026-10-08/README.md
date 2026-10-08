# SF01検証artifact

[結果](../../../docs/internal/security-foundation-sf01-results.md)・[review台帳](../../../docs/internal/security-foundation/sf01-review-log.md)。`provenance.json`にsource head/hash、base/RED commit、cache/toolchain条件を保存する。ログは歴史的RED/途中失敗と修正後GREENを混ぜない。snapshotは検証履歴で、productionへコピーしない。

`logs/`は訂正RED、runtime API未定義compile RED、期限後Err RED、レビューcapability RED、目的別修正後、旧Task署名で止まったworkspace、clippy/fuzz/site/path検査。`independent-review-d127b28/`は独立check成功/Rust拒否とshared native受理の原証拠。`application-example-verification/`/`library-example-verification/`はnative-backed verifierのcheck/build/smoke原log/JSON。終了statusと期待は結果資料に記録。全workspaceの成功原ログと[最終独立再確認](independent-review-f9ec01d-final/README.md)を保存済み。[source f9ec01dの4 OS/全必須CI読戻し](ci-f9ec01d/readback.json)と各platformのSF01 command/result抜粋も保存済み。fullCI出力ではなくSF01二stepの範囲抜粋で、改行はLFへ統一した。

実行済みbinaryコピーはbytes/hashを`omitted-binaries.json`へ保存してGit同梱を省略。source/provenance/結果は保持し、binaryの再実行やclean buildを主張しない。

全locale/全Rust/任意Drop/攻撃耐性の普遍証明ではない。外部サービス/production secrets/第三者攻撃/資源枯渇を測定しない。歴史的source snapshot内の相対linkは元repository contextを前提とし、maintained Docsの検査と分ける。

Repeated generated Rust diagnostic line maps (`.nagi/.../provenance.json`, containing only `rust_lines` and `schema_version`) are omitted after recording their original path, bytes, SHA-256, and entry count in `omitted-generated-line-maps.json`. Generated Rust/Low, original execution logs, measurement records, and source/environment provenance are retained. Archived generation directories are evidence, not runnable distributions.
