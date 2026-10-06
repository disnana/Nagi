# PR #87公開move DocsとS1方針の独立Solレビュー

2026-10-06、read-only。公開head `e4089735de35e5f8496a1191b870e7b5f7edffc9`とlocal `1727679c7f05410f38d4d6a9c287fa272fc6c529`のtree `b27db5899e373fdc1c2d3418c4df004a3462bf77`は一致。AGENTS、ownership・language-guide・modules-and-rust各日英、DESIGN日英、ADR011、OWN-04、checkerとexplicit_movesの関連oracleを照合した。次工程は別worktreeのTask設計・独立レビューを参照した。編集・Cargo・GitHub mutationなし。このレビューは品質の保証や新しい実行検証ではない。

## #87公開Docs

重大な意味論ミス、移行範囲の拡大、関連箇所の日英差は見つからなかった。以下は公開前blockerではなく、小さい説明改善案。

- moveは所有値とcleanup責任の引渡し、操作そのものはclose/破棄ではないと説明済み。Python参照代入との違い、viewによる借用、copyの独立複製、shared handleの追加を分けている。
- 所有するnonCopy既存Localそのものが通常代入RHSのときだけ明示操作を要求する。新値生成・引数・return・field/index・try/matchのconsumeは維持と日英で記載。所有fieldの部分move例が残っていることも、この狭い移行と整合する。借用/shared親のnonCopy field取得は禁止を維持。
- move後の元変数は非Copy入力なら使えず、再初期化後は使える。ownershipの数値/文字列/再初期化例、guideの意図的なuse-after-move例がある。Copy表は既存checkerと一致し、Result/sharedはCopy payloadでも非Copy。viewのCopyは借用元を不要にせず、ローカルasync関数別名のCopyをFuture保存と混同していない。
- canonical import/qualified/alias、ユーザー同名関数は通常関数、one argument/inferred/no explicit type arguments、Future/nested Future拒否、作業branch実装済み未リリースの説明も一致。

### 最小改善案（二件）

1. **Copy入力のmove後について一文を補う。** `docs/ownership.md:43` / `docs/en/ownership.md:43`に「Copy値にmoveを書く必要はありません。書いても元の変数は引き続き使えます」/ “Copy values need no move call; using one still leaves the original variable available.” を足せばよい。現状はCopy対象表は正しいが、一般語としての「move後は使えない」を全入力へ広げる初心者の余地がある。根拠は `compiler/src/check.rs:2361` のCopy consume分岐と `compiler/tests/explicit_moves.rs:285` の18型対（move(value)後にreturn value）。新Copy政策や新テストは不要。
2. **guideのcopy提案を代入の置換と書く。** `docs/language-guide.md:167` / `docs/en/language-guide.md:167`の「元の文字列も読むならcopy」を、「元のnameも使うなら、destination = move(name)をdestination = copy(view(name))に置き換える」と短く具体化する。move後へcopy行を追加すると元nameは既に使用不可である。ownership側の説明は既にdestinationへの代入式を提示しており、guideもそれへ揃えるだけでよい。

bare assignmentとpost-move useのコメントを既存例へ一行ずつ足す案もあるが、現状に意図的拒否例・結果・修正があるため新しい完全プログラムを増やす必要はない。modules/Rustのalias例は十分短く、既存consumeを変更していない説明も正しい。`print(name)`/`len(name)`は読むだけだが、`print(move(name))`/`len(move(name))`なら先に明示transferが起きる。実装はby-value identityを生成するため、将来これを単なるannotationやborrow透過と説明しないこと（今回のDocsにその誤記はない）。

## S1: 今回の自律判断権限による見直し

結論: **全T正常出口await/discardとsticky faultのA+Aを、理由付きの初版方針として自律採用するのが妥当。必須質問へ戻す必要はない。** 既承認の大枠だけから一意に導けないことは変わらないが、今回の「既存設計から安全に判断できるものは理由を示して自律確定」という委任は、新Taskに限った一貫した初版方針を選ぶ根拠になる。以前Stopにした履歴だけを今回の確認必須理由にしない。採用するなら新しい詳細判断として設計へ理由・範囲を記録し、tests-onlyのoracleを固定してから実装する。

### 全Tの正常出口awaitまたは明示discard

- **安全性から絶対必須ではない。** Scopeが全actual joinを持つならhandle暗黙Dropでも子をdetachする必要はない。at-most-onceとmust-consumeは別であり、既存の所有local/Result binding全体にmust-use義務を足す根拠はない。
- **初版として全T同じ規則を選べる。** 新Taskの結果bindingは結果を受け取る意図を表す。unit/Copy/nonCopy/Resultで処理義務を分けない方が、返却型変更で無言の放棄へ変わらず、明示discardにより不要という意図も書ける。結果不要の旧statement spawnは残るため、既存プログラムにこの義務を遡及させない。
- 正常binding/scope出口、継続branch、loop backedgeだけのTask義務として記録し、move aliasで移す。body Err/panic/親取消はcleanupへ渡す。receive Future作成でhandleをconsumeし、取消で復活させない。一般owned型やawait後の内側Resultの未使用検出を広げない。
- 代替はTaskの暗黙Drop許可。実装は小さいが未受取業務Resultも黙って落とせる。Resultだけ義務化は型依存の例外が増える。Aを選んでも業務Errを必ず処理する保証や任意T close完了にはならない。

### fault処理後もScopeのprimaryをstickyに保持

- **現runtimeから必然ではない。** `runtime/src/concurrent.rs:23`はsticky fieldを持たず、fault joinでshutdown→Errの後は空setで再join Okになり得る。現Nagiは出口の一回joinなのでこの回復を公開していない。
- **初版はstickyが自然。** 新receiveでfaultを先に観測すると、そのrecordを消して正常出口にすると既存scopeのfail-fastを偶然弱める。既に兄弟abortしたscopeを、受取Errを表示しただけで健康に戻す操作も設計に無い。故障の一次原因を保存し、再join/receive/出口が同じ故障を扱う方が、Scopeの寿命・故障ownerという採用方向に合う。
- 普通の子Result Errは内側業務値のままで兄弟継続。panic/予期しない取消/legacy statement Err/protocol故障はfaultとして兄弟abort→全actual join後に外側Err。bodyの元Errは後続child faultで置換しない。native Cancelledから真の時系列を推測せず、owner別abort要求を記録する。parent Drop/unwindはabort要求までで、全join完了を返さない。
- 代替の明示受取fault回復は合理的だが、兄弟を継続するか、abort後のack、未観測fault、legacy Err、復旧のownerを追加設計する必要がある。初版に暗黙回復を入れず、復旧は新Scope/明示Supervisor方針へ分ける。stickyはbodyへの強制割込み、全faultの即時発見、副作用rollbackを保証しない。

### 採用時に維持する検証・非対象

- Task設計のscope内ticket＋未join native ID対応、偽ID再利用oracle、唯一JoinSet owner、Ready→record間await無し、receiptのScope強参照無しを維持する。
- 全Tの正常放置拒否/await-discard受理をHigh/保存Low/手Lowとbranch/loopで対にする。faultをmatch後にもscope出口Err、業務Errなら健康兄弟barrier、body Err優先、receive/drain取消後のrecord保持を独立観測する。sleepで順序を作らない。
- 旧Supervisor terminal→HTTP取消を新Task内側Resultへの機械置換で失わない。Tのchild側送信失敗Dropとparent側buffer Drop、未join/未受取保持を別に測る。Drop panicからの普遍回復、capacity/timeout、公開SQLite、全spawn移行は追加しない。
- これらは方針選択の提案で、Task実装済み・CI成功・main反映ではない。#87公開headは変更しない。今回の読取レビューを実証や品質の保証とはしない。
