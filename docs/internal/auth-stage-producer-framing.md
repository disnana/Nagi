# Session auth-stage診断のproducer行境界

対象sourceはPR #150のremote `e4490dc99decd1dae9db39e8f4307b659bb5da9f`と同じtree
`4360ecb064327e4d0b8df7821af1d2667b5294f9`を持つlocal `bc34474b`。

## 観測と未確定範囲

Linux job `120346710934`の通常検査は成功したが、Session補助診断の最終gateが失敗した。
公開された正規化traceはbroadのcargo exit 0 / inventory passを記録し、slot 2だけを残して
`trace=unavailable` / `trace_validation=failed`となった。原rawはworkflowが削除しており、
今回のCIで何がheaderを失わせたかは確定できない。

source上は両対象testの成功末尾でGuardをemitする。producerは有限bufferをstderrへ書き、
workflowはlibtestのstdoutとstderrを`2>&1`で同じrawへ集める。一方、parserは行頭からの
完全一致headerだけを受理する。改行のないlibtest進捗prefixにslot 1 headerが続く合成例は、
正しいrecordが続いても「slot 2だけ＋unavailable＋validation failed」を再現した。
これは可能な欠落経路の反例であり、削除済みCI rawの原因証明ではない。

## 採用した限定修正

Guardの既存render処理をprivate `encode`へ機械的に分け、Guardは返された同じbytesを従来どおり
`stderr.write_all`へ渡す。headerの先頭に改行1byteを含め、改行のない先行出力とheaderが同じ行に
なることを避ける。後処理でheaderを探し直す処理や、parserの寛容化は加えない。

改行も既存buffer budgetへ算入する。`OUTPUT=8192`、footer予約64bytes、local 24 / global 64 records、
raw/outputの上限は変更しない。既存上限へ1byte近づくため境界のrecordが収まらなくなる場合も、
従来の`truncated` / `lost=true`により診断を拒否する。上限は増やさない。
collectorのtry-lock、競合/poison/record loss、emit/Drop、stderrの書込み方式は変えない。

追加のthread・process・lock、共有slotの再利用、期限・並列度の変更はない。test-only cfgの範囲であり、
通常runtimeのSession/CSRF/PreLogin、認証意味論・失敗・cleanupは変更しない。
必要slot `{1,2}`、closed records、cargo exit、inventory、native coverageは引き続き必須。
任意の混線・短い書込み・欠落でも必ず完全な診断を得られるという保証は加えず、不完全な証拠は拒否する。

stdout/stderrを別fileへ分ける案なら混線を構造的に避けられるが、workflow両系統、inventory/
panic位置/browser coverage、read budget、cleanupの設計変更が必要になる。今回はそれらを変更せず、
producer framingの最小変更を採った。

## RED / GREENと必須回帰

Rustの新しいtestはglobal SLOTSを使わず、ローカルCollectorと実Guard encoderのbytesを使う。
改行のないlibtest prefixへ実出力を連結し、headerが必ず次の行に始まることを確認する。
encode抽出だけの旧headerでassertがREDとなり、先頭改行を追加してGREENとなった。
実際のemitとtestは同じencoderを使用する。

通常の`lost=false`、local loss、encoder overflow、collector競合時の`lost=true`、header/footer、
追加改行を含む厳密なbyte境界を検査する。最初の新overflow fixtureでは短い64 recordsが実際には
buffer内に収まったため、最大幅の固定数値recordへ修正した。その失敗logも保持し、既存assertは変えていない。

Pythonは改行で区切った両slotを受理し、改行のないprefixを依然拒否する。slot欠落・重複・未知slot、
不正record、private suffix、未閉鎖、`lost=true`も拒否し、元prefixやprivate textを公開しない。
strict parser自体は無変更である。

## 検証結果と証拠

実encoder framingのRED（cargo exit 101）、producer単体4 testsのGREEN、Python診断41 testsの成功を確認した。
現在のworkflowと同じfocused/broad/list/normalizerはすべてexit 0。focusedはslot 1 / 27 recordsと
slot 2 / 19 records、broadは実際の並列順slot 2→1で46 recordsと2 closed footersを保存し、
unavailable/validation failure/lossはない。broad runtimeは909 passed / 0 failed / 既存1 ignored、
現在のrequired native coverageは17/17成功。関連browser-stage Python 9、Session deadline Python 65とfmtも成功した。

追加で実行したbothdiag cfg付きClippyは、変更外の`session_deadline_observer.rs:303`の未使用`emit`と
`session_native_finish_tests.rs:27`の定数assertで失敗した。その原logは保持し、この変更で修正・lint抑制を
行っていない。該当2fileのbaselineとのdiffゼロを確認したが、baselineのcfg付きClippy再実行はしていない。
通常CIと同じdiagなしの`cargo clippy --locked -p nagi-runtime --all-targets -- -D warnings`は成功した。
原raw・失敗logは専有の`local-only/ready-ci-auth-diagnostic/framing/`へ私有保存し、公開するのは有限traceと
固定scalar・source hashだけとする。raw、path、panic payloadをartifact/PRへコピーしない。

独立High最終auditと最新HEAD CIは別gateであり、ここでの局所RED/GREENをCI失敗の原因確定や
最新HEADの全必須検査成功として扱わない。
