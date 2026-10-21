# nextest の任意評価

この workflow は、既存の `nagic` 統合テスト `enum_parser` と `builtin_type_contracts` だけを、Linux x86_64、Windows x86_64、macOS ARM64、macOS Intel で比較します。同一リポジトリの PR に `ci-nextest-benchmark` ラベルが付いた場合、または `main` から手動実行した場合だけ開始します。通常の CI テストを置き換えたり、必須ゲートにしたりしません。

最初に `cargo test --locked --no-run` で対象をビルドし、ignore テストを含む libtest の全一覧、その ignore 専用一覧、nextest の JSON 一覧を照合します。テスト名と ignore 状態が完全一致しなければ、実行時間を測らずに失敗します。nextest 0.9.148 は checksum 検証を有効にし、fallback を無効にして導入します。

各 OS で、各ランナーを1回ずつ実行する4組のウォーム比較を順序交代しながら測ります。OS ごとに先行ランナーも交互にし、各組では直前の組と順序を逆転します。Cargo の `--locked`、既定 feature、既定の並列度を保ちます。nextest は fail-fast を無効にし、retry は 0、空の実行は失敗にします。対象のビルド準備、一覧取得、各テスト実行の時間を分けて記録します。Cargo cache action は設定せず、nextest と Rust toolchain のセットアップは Actions の別ステップです。セットアップの各ステップ時間は workflow UI で確認でき、helper の JSON artifact には含めません。

対象はこの 2 つの統合テストだけです。nextest は doctest を実行しないため、既存の `cargo test --doc` は維持します。評価結果は時間の観測であり、成功しても高速化を証明せず、必須 CI の変更も許可しません。rustc を呼び出す `expression_contracts`、native/socket/browser テストは対象外です。

helper と unit test は Python 標準ライブラリのみを使います。test file は `scripts/ci` にあるため、通常の早い検査 `python -m unittest discover -s scripts/ci -p 'test_*.py'` にも含まれます。任意 workflow では対象 test だけを実行します。ローカル検証は fixture と YAML 構文だけで、Rust のビルドや nextest の導入は行いません。4 OSの実測結果は以下の通りです。

JSONのschemaは [0.9.148の一次source](https://github.com/nextest-rs/nextest/blob/cargo-nextest-0.9.148/nextest-metadata/src/test_list.rs) のTestListSummary/RustTestSuiteSummary/RustTestCaseSummary/FilterMatchと照合した。fixture検査と実nextest出力の観測は区別します。実行時にはnextestの版一致を検査し、Cargo/rustcの実版も記録する。同PR/refの重複実験はconcurrencyで取消す。

## 2026-10-20の実測

[run 39514555992](https://github.com/disnana/Nagi/actions/runs/39514555992)（HEAD `dde28e1`）は4 OSとも成功し、同じtest/ignore集合を確認しました。[測定値](../../benchmarks/results/ci-optimization-2026-10-20/nextest.json)は各runnerの4回のwarm実行です。中央値は秒で、Cargo起動とtest processの経過時間を含みます。

| CI runner | Cargo/libtest中央値 | nextest中央値 |
|---|---:|---:|
| ubuntu-latest | 0.2951 | 0.4762 |
| windows-latest | 0.4070 | 0.7265 |
| macos-15 | 0.3418 | 0.6670 |
| macos-15-intel | 0.5332 | 1.0214 |

この軽い2対象ではnextestが約1.6〜2.0倍長くかかりました。必須CIはCargoを維持し、nextestは任意評価に留めます。全回帰・native・socketテストの結果や一般的なnextestの性能には拡張しません。build時間は依存解決・downloadも含み、setup時間、runner待ち時間、RSS、請求額はこのwarm中央値に含みません。
