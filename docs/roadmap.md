# 継続開発

当面の目標は、読みやすいHighで一般的なバックエンドを書ける範囲を広げ、型・所有権・失敗・資源の終了を一貫して扱えるようにすることです。Rust経由のネイティブ生成と既存ライブラリを基盤にします。

このページは今後の開発で優先する課題をまとめています。リリース時期は未定です。現在使えるAPIは[リファレンス](README.md)、版ごとの変更は[CHANGELOG](../CHANGELOG.md)で確認できます。

## 優先する課題

1. **既存の言語規則を揃える。** `owned`・`view`・move、分岐やloopの検査、算術の失敗、source mappingを整理する。`check`成功後のRustビルド失敗を再現例で追い、Nagi側で診断すべきものとRustへ任せる検査を区別する。
2. **Rust資源との境界を決める。** 不透明な型の識別、借用、共有、非同期終了、取消後の仕事を定義する。一般的なtraitや新構文は、具体的なAPIで必要性を確認してから検討する。
3. **標準ライブラリを整理する。** HTTP・JSON・DB等のAPIをmoduleとして提供し、不要なランタイム依存を分離する。生成するSerde・行変換・公開型にも対応が必要で、Cargoのoptional化だけでは終わらない。
4. **DBを汎用化する。** SQLiteとPostgreSQLは別module・別資源型とし、引数・行・エラーの規則を揃える。SQLite Pool/Txの公開APIと終了方針は#99としてmainへ反映し、4 OS CIを確認済みです。公開Nagi 0.1.11には未収録です。使い方と制約は[SQLite PoolとTransaction](sqlite-pool.md)を参照してください。Nagi 0.1.10の[明示SQL/schema検査](sql-check.md)はSQLiteの名前・返却列・bind数が対象で、値の型・NULLや他DBの検査は未対応。
5. **HTTP基盤を比較する。** Axum／Towerを第一候補として、現行実装と同じAPI、制限、障害、停止条件で評価する。通常負荷・過負荷・長時間の性能と保守負担を確認して採否を決める。置き換えは未決。
6. **実例と文書で境界を確かめる。** Nagi・Rust双方の検査、公開版とmainの差、実測条件を示す。診断や書きやすさも、同じ課題を解く実例で評価する。

詳細は[ライブラリとRust連携の設計案](library-design.md)へ。採用済みのSQLite初版契約は[ADR 010](internal/adr/010-sqlite-transaction-boundary.md)にあります。その他の資源のAPI名や終了契約には未確定の部分があります。

既存所有値の代入に明示moveを要求する狭い移行、spawn結果handle、業務Errとtask故障の分離はNagi 0.1.11で公開済みです。Supervisor/HTTPのS2移行も既存Task APIを使って実装し、monitorの内側Errを親bodyの`try`へ渡します。S2もNagi 0.1.11で公開済みです。利用するcompilerの版を確認してください。条件付きshared actor messageは後続の設計対象です。公開版との差、未決の細部、移行条件は[DESIGN](../DESIGN.md)と[ADR 011](internal/adr/011-language-behavior-and-docs.md)へまとめています。

処理系の検証は、既知のpass/fail例に加え、High・保存Low・生成Rustを通す小さい生成テストと変異試験で続けます。検査した範囲と未対応の組合せを分け、受理後の生成ミスを回帰例へ残します。

## Lowの範囲

既存のLowコード、波括弧構文、生成物の確認、関数差し替えは維持します。当面はHighとRust連携を優先し、Lowのpointer・layout・unsafe・C ABI・SIMDを独立した低水準言語として拡張する計画は進めません。

この制限と現在の使い方は[HighとLow](low-language.md)に記載しています。

## self-hosting

独自バックエンド、self-hosting、独自VM・scheduler、無停止更新、分散actorは未着手です。今後採用するかも未決です。

Rust以外のバックエンドでも同じ意味を保つには、型・所有権・解放・失敗・非同期処理・runtime接続の契約と実装が必要です。Lowを使ってコンパイラを書き直すことを、現在の開発段階の必須条件にはしません。

S1 Task結果handleとS2 Supervisor/HTTP移行はNagi 0.1.11で公開済みです。S2は既存APIで実装しています。[使い方](task-handles.md)と[検証状況](internal/task-handles-s1-results.md)を分けています。Supervisor/HTTPのS2移行は[サービス例](../test-nagi-code/library-examples/supervised-service/README.md)へ接続しました。公開Pool/Txはmainへ反映済みで、0.1.11には未収録です。

## 0.2.0 Security Foundation

次工程の[設計RFC](internal/security-foundation/rfc.md)と[機能別PR/検証計画](internal/security-foundation/implementation-plan.md)を作成しました。認証・認可、CSRF、HTML出力、SQL構造/bind、送信先検証、CORS、Cookie/Session、資源上限の責務を分けます。現在は設計段階で、新機能を利用できるという案内ではありません。最新指示による[安全性優先の判断と移行](internal/security-foundation/decisions-and-migration.md)を基準に、全標準HTTP policy必須・単一request-bound Grant・永続Sessionを実装し、統合検証前に正式0.2.0を公開しません。
