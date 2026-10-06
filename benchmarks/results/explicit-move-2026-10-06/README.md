# 明示moveの検証記録

2026-10-06、Linux x86_64の共有cloud環境。Rust 1.98.1。実装と保証範囲は[実装結果](../../../docs/internal/explicit-move-results.md)にまとめた。未マージ・未リリースの作業記録であり、ここにある成功件数を言語全体の正しさの証明とはしない。

## 値として移す生成

`materialization/`に括弧、block、tuple、Rust標準identityの小さい比較sourceと原ログを保存した。括弧では値を読むAPIが元placeを借りるため、moveしたはずのVecの破棄が遅れた。blockはtemporary Stringのviewを早く無効にした。採用した`std::convert::identity`はby-valueで一度渡す。元の失敗と試行案も`audit/`に残した。

## 同じ入力場所での比較

`cost-comparison/`の旧暗黙moveと新しい明示moveは、同じphysical source/out、同じRust adapter、同じCargo依存cacheを使った。旧compilerはmain `e7aff1d`から保存したもの。新compilerのSHA-256、source/adapterのSHA-256、全観測値は[results.json](cost-comparison/results.json)にある。生成Rust/Lowも両方保存した。

| 観測 | 旧暗黙move | 新しい明示move |
|---|---:|---:|
| 対象Future | 96 byte | 96 byte |
| native binary | 459224 byte | 459224 byte |
| checkの中央値（warmup後7回） | 6.47 ms | 11.30 ms |
| warm cacheのbuild 1回 | 205 ms | 198 ms |

checkには新しい標準moduleのimportも含む。共有マシン上で旧→新の順に測ったため、各値からmove単体の時間や速度向上は判断できない。check時間の増加は残る観測。main統合前の6.71／17.17 msの観測も`first-same-path-observation.json`へ保存し、都合のよい測定だけを採用していない。throughputや全allocationの比較ではない。cost reportは静的site数で、動的allocation数ではない。

再確認する場合は各folderの`main.nagi`と`native.rs`を同じ入力先へ順に置き、対応するcompilerで`nagic build main.nagi --no-project --rust native.rs --out generated --cost-report`を実行する。生成nativeを起動してFutureサイズを読み、checkをwarmup後7回測る。旧compilerと新compilerを混ぜず、Cargo/Rustとprofile/target条件を固定する。

## 以前のFuture +32 byteの比較

`view-storage/`のHigh・保存Low・手書きLowは同じ観測だった。

| 対象 | Future |
|---|---:|
| Nagi生成 | 168 byte |
| caller由来のviewだけを保持するRust参照 | 136 byte |
| 同じlocal→caller借用履歴・cleanup位置・extern bridgeのRust参照 | 168 byte |
| 同じ履歴でnative pauseを直接awaitするRust参照 | 160 byte |

最初の参照は借用履歴とawait bridgeが異なる。条件を揃えた参照との差はこのケースでは0 byteで、+32 byteをNagiだけの余計な配置と断定しない。bridgeを変えた場合の差は8 byteだった。frameの具体的なfield配置、他のFuture、他targetには一般化しない。元の32 byte観測を削除していない。sourceとstdoutを残し、既存`view_container_drop`でallocationとcleanupの有限ケースを同時に検査する。

## 並列installation fixture

`install-fd-probe/`はcopy直後のexecと、読取り専用seedからhard linkしたexecの小Rust比較。copy側は800回中205回`ETXTBSY`で失敗し、hard link側は800回とも成功した。具体的なkernel/FD継承機構は未確認。fixture修正はcompilerの配布動作を変えず、起動retryや全テスト直列化も追加していない。

## 回帰と探索

[verification.json](verification.json)はコマンド、source commit/tree、終了code、時間、原ログhashを保存する。失敗した実行と後の成功を別recordにした。原ログの一部を`audit/`へ収録し、生成Rustそのものを修正して成功させていない。

原ログと生成出力の末尾空白・空行は観測したbytesのまま保存した。コード・テスト・文書のdiff検査と分け、hashを合わせるために原出力を整形しない。

専用`explicit_moves`は9群。正常nativeはHigh・元Highを消して読み直した保存Low・独立手書きLowを各3群、計9 build/runで確認する。compile-failは型・使用後・borrow/shared/temporary・分岐/loop・定数演算と元行を確認する。固定corpusは42 source、限定生成は18 grammar。追加探索は256生成caseと、10,000 text mutation／128 native caseを使った。任意text mutationのnative compileは行わず、受理後Low再check/emitまでと、限定生成のrustc/native oracleを分ける。coverage-guided fuzzingではない。
