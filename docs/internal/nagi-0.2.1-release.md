# Nagi 0.2.1 正式公開準備

2026-10-22。main `f75300a`に統合済みのビルド・安定性改善を0.2.1として公開する。新しい最適化やIDEの実行設定変更を追加せず、workspace版と稼働するlock内のlocal package版、CHANGELOGを揃える。外部依存版・release profile・High/Low意味論・安全検査は変更しない。

初回HEAD `c640223`の正式CI `39779857773`は、Windowsのsupervised-worker High nativeで終了イベント数のassertに失敗した。実数は記録されず、他3OS配布と全OS regressionの成功もWindows必須gateの代わりにならない。調査で別途確定した「stop要求後、SHUTDOWN公開前にevent streamが閉じる競合」を小さいruntime修正として追加した。[設計と決定的RED/GREEN](supervisor-shutdown-publication.md)を参照する。終了通知0件の再現から1件へ修正し、既存join・取消・STOPPED処理・期限を維持した。元Windows失敗との同根性を断定せず、改訂HEADの全4OS CIを公開条件とする。

## 対象と保証

- 開発時は`nagic run app.nagi --profile dev`を選べる。CLI既定はreleaseのまま。初回はRust依存のコンパイルが必要で、すべてのcold buildを瞬時にする変更ではない。
- 生成本体CASと関数unitsはopt-in。完全な型・所有権・capability検査とCargo fingerprint判定を維持し、不明な境界では通常生成へ戻す。unitsは編集時に遅い測定結果もあり既定にしない。
- 共有Cargo target、immutable世代、旧成功exe、lock継承、失敗時の旧latest保護を維持。`cache inspect`は有限の読み取り専用観測で、削除・quota・回収可能容量の証明ではない。
- Tokioは1.54.1。Nagi本体更新だけでは既存アプリの独立lock・実行ファイルを更新しない。新runtimeと必要な依存解決で再ビルドする。

実装・測定・残る範囲は[0.2.1作業記録](release-0.2.1-build-stability.md)と[測定結果](../../benchmarks/results/release-0.2.1-build-stability-2026-10-22/README.md)を参照する。コンパイラquery cache、テスト結果cache、全async関数のunits分割、自動粒度最適化は今回の完成機能ではない。

## 最終gate

版更新のlock整合性・配布policy・component別release planを検査し、変更後HEADの独立auditと必須CIを確認する。正式Nagi版変更なのでLinux x86_64・Windows x86_64・macOS ARM64・macOS Intelの全4配布を対象とし、抽出した配布物・High/保存Low/手書きLow・native・installer・security gateを省略しない。過去の3OS PR成功を4OS正式版検証へ付け替えない。

公開は既存main workflowを使用する。全gate成功前にversion/tagの存在だけで公開済みとしない。公開後は`nagi-v0.2.1`のcommit、draft/prerelease状態、4 archive/checksum、compiler表示版とruntime/付属toolを読み戻す。GitHubのLatestがNagiを指すことも確認する。JetBrainsとVS Codeの版・Marketplace提出はこのPRで変更しない。

JetBrainsの通常Runをdev既定、明示選択をreleaseにする改善は別PR。公開0.2.0へ未対応flagを送らず、コンパイラ対応・取消・trust・save-before-runを維持する。0.2.1公開をその追加実装の完了待ちにしない。
