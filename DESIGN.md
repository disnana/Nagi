# Nagiの目的と設計判断

[English](DESIGN.en.md)

Nagiは、Rustの性能とライブラリを使い、HTTP・認証・認可・validation・DB・データ処理の境界を簡潔に書く言語を目指します。主な処理は読みやすいHighで書き、ライブラリ固有の設定や高度な機能にはRustアダプターを使います。RustやGoの置換、独自HTTP・DB基盤の再実装は目標にしません。

連携の手間を減らすことと、失敗の理由をNagiのコードから理解できることを重視します。任意のRust APIをそのまま使えることや、Rustの知識が一切要らないことを、現在の機能として約束するものではありません。

この文書には、現在の実装と、採用・保留した設計方針をまとめています。次フェーズの変更は未リリースで、PR #74とは別に検証します。使い方は[リファレンス](docs/README.md)、今後の順序は[roadmap](docs/roadmap.md)、公開版の変更は[CHANGELOG](CHANGELOG.md)で確認できます。

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
| Rust資産との共存 | アプリの処理をHighで書き、既存framework・driver・独自基盤を組み合わせる | Axumとの双方向async連携をサンプルで検証。一般のasync callback型は未対応 |
| Low | 既存互換性、波括弧構文、生成内容の確認、関数差し替えに範囲を絞る | 実装済み。独立した低水準言語への拡張は当面進めない |
| 実行ファイルの生成 | 現在のRust backendを使う | Rust/Cargoによるネイティブ生成を実装済み |
| DBの拡張 | SQLiteとPostgreSQLの型を分け、操作・行・エラーの規則を揃える | SQLiteの初版API・終了契約は承認済み、公開APIは未実装。PostgreSQLは後続設計 |
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

サポートするNagiコードを受理した後、Nagiで検出可能だった型・move・lifetime問題で生成Rustが拒否されるのはコンパイラの不具合です。Rustへ委譲するcrateのAPI・trait・手書きRust・ビルド環境の検査とは区別します。[言語契約](docs/internal/language-invariants.md)、[生成経路の比較](docs/internal/compiler-pipeline.md)、[テスト基盤](docs/internal/compiler-testing.md)に責任範囲と確認方法をまとめています。

借用を含む値では、現在の借用元と、値を片付ける位置を分けて扱います。返却につながるList・Result・Optionの生成計画は、checkerが記録したmove・borrow・置換・分岐の出口を使い、元の宣言位置に格納先を置きます。公開型を変えず、私有の`Option<T>`で古い格納先を退役させます。ループには固定点まで検査したbodyと条件を使います。asyncにも同じ計画を使い、scope内の借用を別の生成asyncへ持ち出さないようにします。

これは新しい実行時の所有権管理ではありません。Rustへ参照の検査、move後の破棄判定、unwindとFuture取消時の後始末を任せます。Nagi側は、右辺の評価順、置換時の破棄、元のcleanup位置を保つ責任を持ちます。[生成経路](compiler/tests/view_flow_completion.rs)、[資源の観測](compiler/tests/view_container_drop.rs)、[実scopeの検証](compiler/tests/scope_runtime_contract.rs)で確認します。payloadのコピーは追加しませんが、格納先やFutureの大きさが増える場合があります。任意の資源型や全borrow経路を解決したという意味ではありません。

根拠: [コンパイル経路](compiler/src/emit.rs)、[所有権境界のテスト](compiler/tests/ownership_boundaries.rs)、[ビルド診断のテスト](compiler/tests/build_diagnostics.rs)、[Rust依存設定のテスト](compiler/tests/rust_dependencies.rs)。検査の範囲は[所有権](docs/ownership.md#借用と検査の範囲)を参照してください。

## Rust資産との接続を中心にする

ライブラリを使うアプリ作者と、Rustアダプターを作る作者の両方を支えます。一般的な操作には標準APIを用意し、独自の資産には小さなアダプターを作って再利用できる形を目指します。Rustの型・traitをすべてHighへ公開することや、各crateを専用の標準APIで包むことは、現在決めた方針ではありません。

| 担当 | 内容 |
|---|---|
| Nagi | アプリのclass・enum、検証・計算、Result、asyncの呼び出し、Nagi上の診断 |
| Rustアダプター | 対応する型とエラーの変換、ライブラリ固有の設定、生成されたNagi関数との接続 |
| Rustライブラリ | HTTPの輸送・middleware、DB driver・pool、暗号、OS連携などの実装 |

現在は`nagi.toml`でCargo依存とRustファイルを指定し、`@rust`を付けた`extern`宣言から呼び出します。Rust側から、生成された特定のNagi関数を名前で呼び、async関数をawaitすることもできます。同じCargoビルド内のソース連携で、安定した外部ABIではありません。

[Axum見積API](test-nagi-code/application-examples/axum-service/README.md)では、Rustがroute・JSON入力・応答statusを担当し、Nagiが型付きの数量検証と価格計算を担当します。RustからNagiのasync関数を呼び、NagiもRustのasync処理をawaitします。この経路は固定した関数名と生成型に依存します。任意のasync関数値をexternの引数として渡す機能とは別です。

Rust側で作ったHTTPサーバーには、そのアダプターの制限・停止・panic処理が適用されます。Nagi標準HTTPの設定は自動では適用されません。コピーやserialization、エラー変換もアダプターの実装次第です。

生成Rustの型不一致は、対応するNagiファイル・文の行に戻します。`build/run --rust-diagnostics`では生成Rustの詳細も表示できます。式の厳密な列位置や任意のRust診断への対応は未実装です。手書きRust・依存crateのエラーはRust位置のまま示します。

同じアダプターを別のアプリで再利用できるか、資源の所有・共有・終了をどちらが担当するかは、引き続き検証します。利用者が定義する不透明な資源型、汎用async callback、宣言の自動生成は未実装です。

### 認証・認可の最小実験

未リリースの`std.auth`は、認証済みの`Principal`と権限型・対象に結び付いた`Grant[P]`を扱います。通常のclassは入力やclaimsを表す型として使えますが、構築やJSON復元ができるため、認証成功の証明にはしません。proofはNagiから構築・JSON復元・copy・shared化できず、保護APIへmoveして渡します。

credentialの検証はRustライブラリ、独自の認可や業務ルールはNagiにも置けます。サンプルではAxumのRustアダプターが名前付きNagi async policyを呼び、成功時だけGrantを発行します。JWSの検証はこのverifierを差し替える用途です。サンプルの固定credentialを本番の認証方式とは扱いません。

checkが保証するのは、宣言された保護APIへ必要な権限型のproofを渡し、move後に再利用しないことです。署名・期限・policyの正しさ、全routeへの認証設定、任意RustやSQLによる迂回、responseへの機密情報流出は証明しません。Rust issuerとNagi policyの内容はアプリ側の信頼境界です。詳細は[ADR 001](docs/internal/adr/001-backend-boundaries.md)に記します。

根拠: [Rust依存の読み込み](compiler/src/project.rs)、[externの検査](compiler/src/check.rs)、[Rust生成](compiler/src/emit.rs)、[Rust依存の回帰テスト](compiler/tests/rust_dependencies.rs)、[アプリの検証](scripts/verify_application_examples.py)。手順と対応型は[Rust連携](docs/modules-and-rust.md)を参照してください。

## Lowは必要か

現在のLowは、Highと同じASTと型・所有権規則を使う別構文です。通常のHighの処理経路でも、検査後にLowテキストへ出力し、再解析・再検査してからRustを生成します。Lowへ書き換えるだけで高速化したり、Rustの借用検査を回避したりはできません。

現状の実利は、生成内容をNagiの規則で読めること、元のHighを変えずに関数を差し替えられること、既存のLowコードを動かせることです。`@replace`は引数・戻り値・asyncの署名を確認しますが、元の関数と同じ挙動になることを証明しません。

ただし、普通の処理の改善ならHighの元関数を編集したり、Nagi moduleへ分けたりできます。低レイヤーの実装にはRust連携もあります。これらに対して、Lowが不可欠だと示す根拠は現在ありません。

Lowを維持するには、二つの構文、テキストへの変換と再解析、差し替えの統合、moduleの識別、診断位置の対応を保つ必要があります。現在は生ポインター、layout、unsafe、C ABI、SIMDを扱えず、独自optimizerもありません。保存Lowにはmodule情報を残しますが、元のHighへのsource mapや検査済みの証明状態は残しません。安定した外部IRやABIとも位置付けません。

当面は互換性と既存の用途を維持し、HighとRust連携の整合性を先に固めます。Lowへの投資は、代替手段と比べて調査・変更・再現の手間が減る具体例、利用実績、保守費用で判断します。他の手段でもできること自体は、Lowの価値を否定する理由にはなりません。これはLowの廃止や移行日程を決めたものではありません。

根拠: [parser](compiler/src/parser.rs)、[loweringと再検査](compiler/src/emit.rs)、[差し替えの統合](compiler/src/check.rs)、[High／Lowの境界テスト](compiler/tests/ownership_boundaries.rs)、[保存Lowのmodule識別のテスト](compiler/tests/stdlib_imports.rs)。例と制限は[HighとLow](docs/low-language.md)を参照してください。

現在のcalls図はLowの差し替え本体と内部の直接呼び出しを追い、Rust本体はextern境界まで表示します（[テスト](compiler/tests/graph_relations.rs)）。一方、Highの呼び出しからの定義ジャンプはHighの宣言へ留まり、Lowの置換先へ自動で移動するわけではありません（[テスト](compiler/tests/symbols.rs)）。Rust側のツールも含めた調査のしやすさは、別に評価が必要です。

### Lowで改善したい作業

次の用途で、現在のLowが調査や実装の手間を減らせるか検証します。

- **生成内容を理解する。** 推論された型、解決された定義、明示したview・copyをLowで確認し、Highや生成Rustだけを読む場合より原因を追いやすいか試します。所有権移動や内部コピーをすべて表示する機能はありません。
- **元のHighを保って実装を比較する。** 同じ署名の関数を差し替え、出力、生成Rust、性能を比較します。変更箇所の分離と実験の再現が容易になるかを評価します。Lowへ移すだけの高速化や挙動同値は保証しません。

Highの別実装、ASTの表示、Rustアダプターも比較対象にします。詳細な所有権の可視化や将来のbackend用IRへの発展は別の未実装候補です。現在のLowを安定したIRやbackend独立性の根拠にはしません。

## ライブラリへ任せる部分とNagiが決める部分

標準HTTPの輸送にはHyper、旧HTTP APIにはAxum、非同期実行にはTokio、JSONにはSerde、SQLiteにはrusqliteを使っています。HTTPやDBを独自実装すること自体を目的にはしません。

既存ライブラリを使っても、Nagi側の責任は残ります。どの型を公開するか、引数をmoveするか借りるか、どの失敗をResultへ返すか、取消とcloseで何が終わるかを決める必要があります。Rustの型を単に隠しても、扱いやすいNagi APIになるとは限りません。

現在の標準HTTPは型付きrequest・response・共有state・async handlerを提供します。SQLiteはclassへの行変換を提供しますが、bindは固定形で、SQL文字列や列名を通常の`check`で検査しません。Nagi 0.1.10以降では、schemaを明示して名前・必要な返却列・bind数を確認する[SQLの事前検査](docs/sql-check.md)を使えます。値の型・NULL可否は検査しません。pool・transaction・PostgreSQLの標準APIもありません。新しいDB資源とアダプターの契約は[ライブラリ設計案](docs/library-design.md)で検討します。

Axum／Towerを採用するかは、同じAPI、接続容量、期限、本文上限、panic応答、停止条件で比べてから判断します。AxumもHyperを使うため、Router／middlewareの比較とlistenerの変更を分けます。既存の異なる条件のベンチマークを、採用の根拠にはしません。

根拠: [依存](runtime/Cargo.toml)、[標準HTTP](runtime/src/http_server.rs)、[旧HTTP](runtime/src/http.rs)、[SQLite](runtime/src/database.rs)、[SQL検査](compiler/src/sql_check/mod.rs)と[テスト](compiler/tests/sql_check.rs)、[HTTPの実通信テスト](tests/http_stdlib_integration.py)。

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

コンパイラとRustの境界は、[段階計画](docs/internal/compiler-rust-boundary-plan.md)で整理しています。最終check済みの情報を封印して生成へ渡すCheckedProgram、ビルド世代の分離、資源契約の集約、Pool／Transactionの順に検証します。High→Lowテキスト→再解析とRust backendは維持します。

Phase 1のCheckedProgramはPR #78でmainへ入り、公開版には未反映です。生成側で型や借用を再推論せず、封印時に確定したplanを使います。実装は[最終factory](compiler/src/check/checked.rs)、検証は[封印境界のテスト](compiler/src/check/checked_tests.rs)と[ADR 006](docs/internal/adr/006-sealed-codegen-input.md)を参照してください。

Phase 2の開発差分では、アプリIDと成功世代を分けます。同じ生成先のwriterはOS lockで直列化し、世代固有のCargo binをbuildしてからexeをコピーします。成功時だけlatestを更新し、旧exeは上書き・削除・killしません。依存キャッシュは共有し、runの前にlockを解放します。生成Low・Rust・manifest・読み取り済みsourceと行対応を世代に保存しますが、外部Rustや依存source全体の原子的snapshot、任意processの隔離、電源断後の耐久性は対象外です。実装は[世代の公開処理](compiler/src/generation.rs)、検証は[実Cargo回帰](compiler/tests/build_generations.rs)、判断は[ADR 007](docs/internal/adr/007-build-generations.md)を参照してください。

Phase 2のPR #79は4 OS・editor/package CIまで成功し、mainへ反映しました。公開版には未反映です。Phase 3の先行テストも4 OSで成功しました。開発差分では、登録資源の型引数の役割とcapabilityの根拠を私有descriptorへ集め、公開ResourceInfoはその一部を参照します。型引数の範囲・重複・欠落を登録時に検査し、用途別の判定と既存APIを保ちます。資源のlifecycle保証はまだ追加しません。[ADR 008](docs/internal/adr/008-resource-contracts.md)に構造と検証の順序を記録しました。集約後の#80は4 OS・editor・site・merge gate CIが成功し、mainへ反映しました。Pool／Transactionはまだ未実装です。[判断記録](docs/internal/open-questions.md)と[進捗](docs/internal/progress.md)で、検証済みの範囲、mainと公開版への反映状況、予定を区別します。

SQLite Pool／Transactionは[APIと終了policyの具体案](docs/internal/sqlite-pool-proposal.md)を用意しました。SQLiteの解析・bind・transactionはrusqliteへ任せ、Nagi側は公開する所有契約とworkerの完了・再利用を扱う案です。汎用引数を用意し、旧Dbの固定bindやSQL制約は変えません。2026-10-06にQ002の公開API・SQL制限・終了契約とruntime rusqlite hooksを承認しました。[ADR 010](docs/internal/adr/010-sqlite-transaction-boundary.md)に従い、一接続・一Txの試作から進めます。pool／workerは既存Rustライブラリと比較して選び、追加wrapper依存は別途判断します。標準APIの実装・検証はまだ完了していません。

開発branchの[private試作](runtime/src/sqlite_prototype/session.rs)では、実SQLiteの禁止action、明示終端、取消後cleanup、native closeとworker joinを[22件のtest](runtime/src/sqlite_prototype/tests.rs)で確認しました。SQL Errだけで変更が戻ったとは扱いません。これは公開Pool、NagiのTx捕捉検査、4 OSでの動作を保証する結果ではありません。[検証結果](docs/internal/sqlite-session-results.md)と[次のadapter判断案](docs/internal/sqlite-pool-adapter-decision.md)に未完了範囲を残します。

意味論、公開API、High／Low／Rustの分担を変える場合は、変更の理由、代替案、互換性、検証結果をこの文書へ反映します。詳細なAPI説明や測定ログは対応する文書に置きます。

未実装の案を実装済みへ変えるときは、実装とテストの参照を追加します。テストがあることを言語全体の保証へ拡大せず、測定していない効果は測定済みとして書きません。議論で合意したことと、動作が検証できたことも区別します。
