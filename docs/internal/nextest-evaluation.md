# nextest の任意評価

この workflow は、既存の `nagic` 統合テスト `enum_parser` と `builtin_type_contracts` だけを、Linux x86_64、Windows x86_64、macOS ARM64、macOS Intel で比較します。同一リポジトリの PR に `ci-nextest-benchmark` ラベルが付いた場合、または `main` から手動実行した場合だけ開始します。通常の CI テストを置き換えたり、必須ゲートにしたりしません。

最初に `cargo test --locked --no-run` で対象をビルドし、ignore テストを含む libtest の全一覧、その ignore 専用一覧、nextest の JSON 一覧を照合します。テスト名と ignore 状態が完全一致しなければ、実行時間を測らずに失敗します。nextest 0.9.148 は checksum 検証を有効にし、fallback を無効にして導入します。

各 OS で、各ランナーを1回ずつ実行する4組のウォーム比較を順序交代しながら測ります。OS ごとに先行ランナーも交互にし、各組では直前の組と順序を逆転します。Cargo の `--locked`、既定 feature、既定の並列度を保ちます。nextest は fail-fast を無効にし、retry は 0、空の実行は失敗にします。対象のビルド準備、一覧取得、各テスト実行の時間を分けて記録します。Cargo cache action は設定せず、nextest と Rust toolchain のセットアップは Actions の別ステップです。セットアップの各ステップ時間は workflow UI で確認でき、helper の JSON artifact には含めません。

対象はこの 2 つの統合テストだけです。nextest は doctest を実行しないため、既存の `cargo test --doc` は維持します。評価結果は時間の観測であり、成功しても高速化を証明せず、必須 CI の変更も許可しません。rustc を呼び出す `expression_contracts`、native/socket/browser テストは対象外です。

helper と unit test は Python 標準ライブラリのみを使います。test file は `scripts/ci` にあるため、通常の早い検査 `python -m unittest discover -s scripts/ci -p 'test_*.py'` にも含まれます。任意 workflow では対象 test だけを実行します。ローカル検証は fixture と YAML 構文だけで、Rust のビルドや nextest の導入は行いません。4 OS の実測は 0.2.0 release CI 完了後、任意 workflow で行います。

JSONのschemaは [0.9.148の一次source](https://github.com/nextest-rs/nextest/blob/cargo-nextest-0.9.148/nextest-metadata/src/test_list.rs) のTestListSummary/RustTestSuiteSummary/RustTestCaseSummary/FilterMatchと照合した。fixture検査と実nextest出力の観測は別で、4 OSの実行は未確認。実行時にはnextestの版一致を検査し、Cargo/rustcの実版も記録する。同PR/refの重複実験はconcurrencyで取消す。
