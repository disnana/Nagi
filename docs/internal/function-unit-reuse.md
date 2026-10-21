# 関数依存単位のCargo再利用（未リリース）

base `0e9a575a28df3d006d5d7d160b0bd2e01113c8db`（PR #139/#140反映後）。ユーザーの「必要なところだけRustを変える」「改善してほしい」に対する第3差分。既存0.2.0の意味論、既定releaseと通常dev、裸の`--incremental`を維持し、明示的な`--profile dev --incremental=units`を追加する。

## 採用設計

毎回High検査→生成Lowの再parse/check→全plan封印を通す。`RustEmission`は全sealed planの整合性を一度検証し、immutable borrowの間だけ複数の選択出力を生成する。ASTを切り貼りして偽のCheckedProgramを作らず、所有権・capability・型の判定をemitterで再実装しない。

初版の単位はprimitiveな同期関数の閉じた直接call graph。署名、binding、expressionの型と対応構文をallowlistし、calleeの解決はsealed FunctionPlanとAST resolutionの一致を要求する。i32→i64の既存変換以外の標準call、間接call、async、extern、属性、ユーザー型、未対応statementは候補から外す。全関数を名前表へ登録し、候補から非候補へ出るedgeがなくなるまで逆依存を反復除外する。primitive名のユーザー型shadowはfail closed。残ったgraphの全edgeが閉じていることを再確認する。

iterative KosarajuでSCCを求め、依存先が先になる連続SCCを最大8個へまとめる。source順ではなくsymbol順でgraphを走査し、同じ単位内の関数もsymbol順で生成する。単位IDはsorted member symbolのhash。16桁ID衝突と、関数・型・root bindingとのalias衝突を拒否する。source/body hashをSCC構成へ入れない。graphや定義集合の変更では構成全体が変わり得る。

各unitは関数と必要unitへのprivate importだけを生成し、型・trait impl・root reexport・entryを生成しない。facadeは全noncandidate/main、全型とtrait impl、元symbolへのpub useを保持し、全unitへ依存する。facade側の関数値・unknown callbackはこの全依存で保守的に扱う。primitive Copy境界のみを跨ぐため、Future frame、Drop、borrow、取消処理を移さない。Rust adapter・追加Rust依存・managed retentionの既存global fallbackは維持する。候補0は本体CASへfallbackする。

## 内容とCargoの境界

各unitとfacadeは既存の`nagi-generated-body-v1\0` content hashを使用し、manifestとRust全byteを含める。manifestの依存先にはruntimeとcallee unitのcanonical path/packageを含む。callee変更はcaller manifest/keyへ伝播する。caller Rustが同じでもCargo再コンパイルが必要になる。native成果物の再利用可否は必ずCargoが判定し、Nagiの意味hashで実行ファイルを再利用しない。Rust monomorphization、metadata、linkの粒度はCargo crateであり、Nagi関数差分と同一ではない。

unitは最大8、facadeを含め最大9ノード。Rust sourceは各64MiB上限。既存CASの専有staging、OS out lock、byte照合、regular-file/link/reparse/exact-layout検査、build/autodiscovery無効、projection identity保護を全ノードへ適用する。未知ファイルを削除して修復しない。新bin名・孤児Cargo隔離・immutable旧exe・publish→projection→atomic latest・run前unlockを変更しない。GCとcross-checkout keyの移植性は追加しない。

互換rootとgeneration rootはruntime・全unit・facadeの同じ依存を持つ。workspace excludeは各CAS pathを明示列挙する。`code/*`のglobではCargoのpath依存自動workspace登録を防げないことを実Cargoで確認したため、両rootの`cargo build --locked`を回帰検査する。

## 診断とreceipt

Cargo artifactのcanonical `target.src_path`が一致するunitだけに、その回のGenerated source mapを使う。facade、unit、別依存/shimを混同しない。provenance.jsonはfacadeの既存rust_linesを保持し、digestごとのunit_rust_linesを追加する。source cache hitでも今回のHigh/import/保存Lowの位置を再生成する。

schema 3はschema 2と同じ固定snapshot artifact集合を使用し、facade digestから参照graphを照合する。Python検証器は既知app/code/digestからpathを導出し、manifestの任意pathへは移動しない。固定table/key・JSON互換一行TOML値・依存alias/package/ID・runtime・全root依存・明示exclude集合を確認する。cycle、同じlogical unitの複数digest、欠損、escape、link/reparse、追加fileを拒否し、最大9ノード/depth9・manifest1MiB/source64MiBを制限する。schema 1/2の既存受入を拡張しない。receiptは署名でもnative入力閉包の証明でもない。

## 検証と範囲

既存generation/diagnostic harnessに、comment・並べ替え・body・公開署名・runtime変更、SCC/bucket/import、mixed facade/async/関数値/型、High/保存Low、旧exe、子CAS追加入力、hardlink、孤児Cargo、公開各段階失敗を追加する。実archive gateは独立unitのFreshを含む3buildと全旧exe・同梱schema 3検証を行う。単なるmock成功をarchive受入に数えない。

`scripts/benchmark_incremental_builds.py`は同一compilerのbody/unitsも比較できる。print-eachとaggregateの2fixtureを分け、実runtime・標準dev profile・同一source/seed lock・独立empty targetで測る。速いfixtureだけを一般化せず、cold・warm・body/API編集・RSS・capacity・native出力を記録する。CI全体時間、production throughput、関数単位機械語差替え、frontend差分検査、test選択/結果cacheは今回の保証に含めない。
