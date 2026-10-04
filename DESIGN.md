# Nagiの目的と設計判断

[English](DESIGN.en.md)

Nagiは、一般的なバックエンドを読みやすいHighで書き、必要なところでRustの資産を使える言語を目指します。HTTP・JSON・DBを使うアプリで、手書きRustを要求する範囲を減らし、失敗の理由をNagiのコードから理解できることが目標です。

この文書は、そのために何を選び、何を保留しているかをまとめます。設計の理由はここ、使い方は[リファレンス](docs/README.md)、今後の順序は[roadmap](docs/roadmap.md)に記載します。実装の有無と判断の採否は別です。以下はリポジトリのソースに対応し、公開Releaseの機能一覧ではありません。

## 対象にする開発

Rustを知らずにAPIを作る人と、Rustのライブラリや独自の基盤を組み合わせる人の両方を対象にします。前者が通常の処理でRustアダプターを書く必要をなくし、後者には高度な処理を追加する入口を残します。

Python風の構文を使いますが、Pythonと同じ動作をする言語ではありません。利用者はNagiの型、move、view、Resultを理解する必要があります。標準PostgreSQL、送信HTTP、汎用DB引数、利用者が定義する不透明な資源型は未実装です。現在は、普通のアプリでもRustへ降りる場面が残っています。

名前は日本語の「凪」です。「内部は激しく動いていても、表面は凪のように穏やか」という考えを込めています。これは設計の方向を表すもので、所有権や障害を利用者から隠す約束ではありません。

## 現在の判断

| 論点 | 判断 | 実装との関係 |
|---|---|---|
| アプリの主な書き方 | Highを中心にする | 字下げ構文、型・所有権の検査を実装済み |
| 一般的な処理 | 既存ライブラリを使うNagi APIで提供する | HTTP・JSON・SQLite等で実装済み。APIの不足は残る |
| 高度な処理 | Rustアダプターとexternで接続する | sync／async連携を実装済み。任意のRust型は直接使えない |
| Low | 既存互換性、波括弧構文、生成内容の確認、関数差し替えに範囲を絞る | 実装済み。独立した低水準言語への拡張は当面進めない |
| 実行ファイルの生成 | 現在のRust backendを使う | Rust/Cargoによるネイティブ生成を実装済み |
| DBの拡張 | SQLiteとPostgreSQLの型を分け、操作・行・エラーの規則を揃える | 新APIは未実装。module名・資源契約は未決 |
| 標準HTTPの基盤 | Axum／Towerを第一候補として比較する | 採用は未決。条件を揃えた比較は未実施 |
| 独自backend・VM・self-hosting | 将来の採否を保留する | 未実装。現在の機能や次のリリースの約束に含めない |

## Highで揃えたいもの

Highでは、日常的なコードの書き方を増やすより、型・データの受け渡し・失敗が同じ規則で読めることを優先します。独自class／enum、nullable、Result、move・view・sharedを使い、業務上の失敗と実行基盤の障害を区別します。

現在も所有する値の受け渡しはmoveが基本で、読むだけならview、元の値を残すならcopyを使えます。ただし、これはランタイム内部のコピーやallocationがすべて明示されるという意味ではありません。例えばHTTP応答の`text`は借りた文字列から応答用の所有値を作ります。

通常のライブラリ機能を追加するたびに、言語のキーワードを増やす方針にはしません。`std.http.server`・`std.actor`はimportできる一方、JSON・DB等には組み込みが残っています。moduleへの分離は未完了です。

根拠: [型・所有権checker](compiler/src/check.rs)、[所有権のテスト](compiler/tests/ownership.rs)、[viewの出所のテスト](compiler/tests/view_origins.rs)、[独自エラーのテスト](compiler/tests/typed_errors.rs)、[標準importのテスト](compiler/tests/stdlib_imports.rs)、[HTTP応答の実装](runtime/src/http_server.rs)。これらのテストは対象ケースの回帰検査で、すべてのプログラムに対する証明ではありません。

## なぜRustを使うのか

Nagi自身が構文解析、名前解決、型・move・viewの検査、Rust生成を担当します。依存のビルド、最終的な借用・trait検査、最適化、機械語生成はRust/Cargoへ任せます。現在もコンパイラを持っていますが、独自の機械語backendは持っていません。

この分担なら、Tokio、Hyper、Serde、rusqliteなどの実装を使いながら、Nagiの構文と公開APIを検証できます。一方、Rust toolchainの準備、Cargoのビルド時間、生成コードとの整合性、Rust固有の診断を扱う負担が生じます。

Nagi checkerを持つ理由は、`nagic check`やエディターで、Nagiの位置にNagiの規則として診断を返すためです。rustcの検査を置き換えるためではありません。現状では、複雑なviewの再代入やRustのtrait境界などで、`check`成功後に`build`が失敗し得ます。逆に、Rustなら有効なコードをNagiが保守的に拒否する場合もあります。

二つのcheckerを持つ以上、その差を把握して保つ費用は避けられません。Nagiの規則を増やすときは、High・Lowの検査結果だけでなく、生成Rustが受理されるかも確認します。独自backendへ進む場合も、今の意味論やランタイムを自動的に引き継げるわけではありません。

根拠: [コンパイル経路](compiler/src/emit.rs)、[所有権境界のテスト](compiler/tests/ownership_boundaries.rs)、[ビルド診断のテスト](compiler/tests/build_diagnostics.rs)、[Rust依存設定のテスト](compiler/tests/rust_dependencies.rs)。検査の範囲は[所有権](docs/ownership.md#借用と検査の範囲)を参照してください。

## Lowは必要か

現在のLowは、Highと同じASTと型・所有権規則を使う別構文です。通常のHighの処理経路でも、検査後にLowテキストへ出力し、再解析・再検査してからRustを生成します。Lowへ書き換えるだけで高速化したり、Rustの借用検査を回避したりはできません。

現状の実利は、生成内容をNagiの規則で読めること、元のHighを変えずに関数を差し替えられること、既存のLowコードを動かせることです。`@replace`は引数・戻り値・asyncの署名を確認しますが、元の関数と同じ挙動になることを証明しません。

ただし、普通の処理の改善ならHighの元関数を編集したり、Nagi moduleへ分けたりできます。低レイヤーの実装にはRust連携もあります。これらに対して、Lowが不可欠だと示す根拠は現在ありません。

Lowを維持するには、二つの構文、テキストへの変換と再解析、差し替えの統合、moduleの識別、診断位置の対応を保つ必要があります。現在は生ポインター、layout、unsafe、C ABI、SIMDを扱えず、独自optimizerもありません。保存Lowにはmodule情報を残しますが、元のHighへのsource mapや検査済みの証明状態は残しません。安定した外部IRやABIとも位置付けません。

当面は互換性と既存の用途を維持し、HighとRust連携の整合性を先に固めます。Lowへの投資は、代替手段と比べて調査・変更・再現の手間が減る具体例、利用実績、保守費用で判断します。他の手段でもできること自体は、Lowの価値を否定する理由にはなりません。これはLowの廃止や移行日程を決めたものではありません。

根拠: [parser](compiler/src/parser.rs)、[loweringと再検査](compiler/src/emit.rs)、[差し替えの統合](compiler/src/check.rs)、[High／Lowの境界テスト](compiler/tests/ownership_boundaries.rs)、[保存Lowのmodule識別のテスト](compiler/tests/stdlib_imports.rs)。例と制限は[HighとLow](docs/low-language.md)を参照してください。

### Lowで改善したい作業

低水準機能の拡張を保留することと、Lowの価値を育てないことは別です。現在の機能を使い、次の用途を検証します。効果を確認した結果や、新機能の採用・日程の約束ではありません。

- **生成内容を理解する。** 推論された型、解決された定義、明示したview・copyをLowで確認し、Highや生成Rustだけを読む場合より原因を追いやすいか試します。所有権移動や内部コピーをすべて表示する機能はありません。
- **元のHighを保って実装を比較する。** 同じ署名の関数を差し替え、出力、生成Rust、性能を比較します。変更箇所の分離と実験の再現が容易になるかを評価します。Lowへ移すだけの高速化や挙動同値は保証しません。

Highの別実装、ASTの表示、Rustアダプターも比較対象にします。詳細な所有権の可視化や将来のbackend用IRへの発展は別の未実装候補です。現在のLowを安定したIRやbackend独立性の根拠にはしません。

## ライブラリへ任せる部分とNagiが決める部分

標準HTTPの輸送にはHyper、旧HTTP APIにはAxum、非同期実行にはTokio、JSONにはSerde、SQLiteにはrusqliteを使っています。HTTPやDBを独自実装すること自体を目的にはしません。

既存ライブラリを使っても、Nagi側の責任は残ります。どの型を公開するか、引数をmoveするか借りるか、どの失敗をResultへ返すか、取消とcloseで何が終わるかを決める必要があります。Rustの型を単に隠しても、扱いやすいNagi APIになるとは限りません。

現在の標準HTTPは型付きrequest・response・共有state・async handlerを提供します。SQLiteはclassへの行変換を提供しますが、bindは固定形で、SQL文字列や列名を通常の`check`で検査しません。pool・transaction・PostgreSQLの標準APIもありません。新しいDB資源とアダプターの契約は[ライブラリ設計案](docs/library-design.md)で検討します。

Axum／Towerを採用するかは、同じAPI、接続容量、期限、本文上限、panic応答、停止条件で比べてから判断します。AxumもHyperを使うため、Router／middlewareの比較とlistenerの変更を分けます。既存の異なる条件のベンチマークを、採用の根拠にはしません。

根拠: [依存](runtime/Cargo.toml)、[標準HTTP](runtime/src/http_server.rs)、[旧HTTP](runtime/src/http.rs)、[SQLite](runtime/src/database.rs)、[HTTPの実通信テスト](tests/http_stdlib_integration.py)。

## 失敗と並行処理の境界

期待される失敗はResult、値がないことはnullableで表します。独自class／enumをエラー型にでき、HTTPでは共通のエラー処理をrouteごとに上書きできます。通常の入力拒否をpanicで表す方針にはしません。

mainのHTTP実装は、応答開始前の捕捉可能なhandler panicを、内部情報を含まない500へ変換します。これは応答途中の回復、abortやOOMからの復旧、状態変更やDB処理のrollbackを保証しません。公開版への収録状況は[CHANGELOG](CHANGELOG.md)で確認します。

scopeとactor／SupervisorはTokio上の同一プロセスで動きます。actorの業務replyでErrを返すことと、worker自体が失敗することは別です。Supervisorの再起動方針は後者へ適用します。scopeの兄弟取消やworkerの終了条件は、アプリの寿命にも影響します。

futureを破棄しても、受理済みのDB処理や外部への送信が取り消されるとは限りません。再起動も業務操作の再実行が安全であることを証明しません。冪等性、transaction、cleanup、資源の寿命は、APIとアプリの両方で扱う必要があります。

独自VM、分散actor、無停止更新は未実装です。Elixir／BEAMと同じ障害隔離や運用機能を持つとは説明しません。

根拠: [scope](runtime/src/concurrent.rs)、[actor／Supervisor](runtime/src/actor.rs)、[lifecycleのテスト](runtime/src/actor/lifecycle_adversarial_tests.rs)、[HTTP panicのテスト](runtime/src/http_server/panic_tests.rs)。契約は[エラー処理](docs/error-handling.md)・[scope](docs/concurrency.md)・[Supervisor](docs/supervisor.md)を参照してください。

## Rust＋Axumに対する価値をどう判断するか

現在Nagi側にあるのは、Highの構文、Nagiの位置で返す型・move・viewの診断、型付きHTTPと独自エラー、scope・actor／SupervisorのAPI、エディター連携、静的なコードマップです。これらの処理をNagiの記述から使えることは実装されていますが、個々の機能がRustで実現できないわけではありません。

Rust＋Axumと比べると、ライブラリの選択肢、型・traitの表現力、資源の扱い、ツールと運用の実績で不足があります。生成コードやRustアダプターを理解する必要がある場面も残っています。現時点で成熟度や性能が上回るとする根拠はありません。

Nagiを使う価値は、同じAPI・DB処理・失敗条件のアプリで、記述量、診断の分かりやすさ、Rustへ降りる頻度、保守負担を確認して判断します。性能は同じ制限と測定条件で、処理量・遅延・CPU・メモリ・過負荷からの回復を比較します。結果が悪ければ、独自実装や抽象化を減らす選択も検討します。

根拠と実例: [symbols](compiler/tests/symbols.rs)、[コードマップ](compiler/tests/graph_relations.rs)、[アプリの検証](scripts/verify_application_examples.py)、[Rust連携サンプル](test-nagi-code/library-examples/README.md)。現行の性能値と条件は[測定結果](PERFORMANCE.md)に記載します。

## この文書を更新する条件

意味論、公開API、High／Low／Rustの分担を変える場合は、変更の理由、代替案、互換性、検証結果をこの文書へ反映します。詳細なAPI説明や測定ログは対応する文書に置きます。

未実装の案を実装済みへ変えるときは、実装とテストの参照を追加します。テストがあることを言語全体の保証へ拡大せず、測定していない効果は測定済みとして書きません。議論で合意したことと、動作が検証できたことも区別します。
