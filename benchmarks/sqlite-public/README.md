# SQLite公開境界の費用比較

同じ`runtime::sqlite`、Options(1 connection / queue 2 / acquire 1000 ms / busy 0 ms)、一つのactive Txで、Nagi→保存Low→生成Rustと手書きRustを比較する。bare rusqliteや旧Dbとの性能比較ではない。

リポジトリrootから次を実行する。出力先は存在しないdirectoryを指定する。依存cacheの準備には通常のRust/Cビルド環境が必要。

```sh
cargo run --locked -p nagic --example sqlite-public-cost -- /tmp/nagi-sqlite-cost
cargo build --offline --release --manifest-path /tmp/nagi-sqlite-cost/generated/Cargo.toml
cargo build --offline --release --manifest-path /tmp/nagi-sqlite-cost/manual/Cargo.toml
/tmp/nagi-sqlite-cost/generated/target/release/sqlite-cost-generated
/tmp/nagi-sqlite-cost/manual/target/release/sqlite-cost-manual
```

共有`CARGO_TARGET_DIR`を指定した場合、実行fileはそのdirectoryの`release/`へ移る。Windowsでは実行名の末尾に`.exe`を付ける。生成packageはworkspaceのCargo.lockを出発点に自身のpackageだけ追加する。既存依存の版が変化していないことを実行記録で確認する。

各操作は4回warmup後、32回だけINSERTを実行する。結果は直接変更数1を検査し、終了時にrollbackとcloseをawaitする。数値thresholdやCI性能合否は設けない。出力JSONには全sample、calling-threadのallocation/byte、未poll Futureの`size_of_val`、lazy openとcloseのallocationを記録する。

native SQLite、worker/observer threadのallocation、全heap保持量はカウンター対象外。時間はworker往復を含む共有hostの一回観測で、throughput・他OS・本番性能を保証しない。SF05では直接literalから作るQueryと、選択済みQuery値の引数渡しを比較する。Queryはopaque Copyで、SQL構造のstring複製を行わない。同条件の手書きRustもQueryを使う。旧Sql::Static/Ownedの測定は歴史的artifactとして保持し、今回のsourceの費用と混同しない。新旧adapterの比較とnative thread/終了数は[公開runtime判断](../../docs/internal/sqlite-public-runtime-decision.md)の別測定を参照する。
