# SQLite private多接続: native容量と独立joinの縦切り

2026-10-06。実装の基点はmain `7999bab`（PR #82）。設計Docsの#83がmain `a3c947f`へ反映された後、その変更も取り込み、合意済みPhase 4を続ける。公開API、Options、数値default、依存、言語意味論は変更しない。対象はcfg(test)のprivate adapterとfixture。公開Pool/Tx、取得期限、compiler capture検査の完成とは扱わない。

## 問題と維持する契約

一接続adapterのlive-empty条件と単一reaperのblocking joinを多接続へそのまま流用すると、健康なAが残るだけでBの取得、終了済みBの観測、次のCの生成を妨げる。#82では多接続の実反例として未確認だった。今回は容量だけを接続した中間source `3a9824d`で、Bの終了確認がAの後ろに止まるbarrier付き回帰の失敗を観測した。公開版の多接続Poolの不具合とは扱わない。

- starting、健康、取消済み、detachedはnativeのlive登録に含む。capacity確認・登録・closing/failed確認は同じledger lockで行う。
- native資源の終了結果を公開し、実JoinHandleのjoin後にだけlive登録を除く。起動不成立の場合はnative未起動と区別し、fake joinを記録しない。
- slot選択・queue・公平性・healthy recycleはdeadpool。SQL/Transaction・cleanupは既存session coreを使い、pool algorithmやSQL parserを追加しない。
- caller取消やclose timeoutで登録/所有責任を失わない。closing/failed後の新登録とBEGINは禁止し、既存active Txは終端を続ける。失敗後の暗黙replacementは行わない。
- 一接続constructorと既存native22/adapter20/比較1の期待値を維持する。複数接続はfilesystem DBとDEFERRED Txを使い、複数`:memory:`を公開契約の成功例にしない。

## 採用する内部構造

workerごとに独立observerを先に起動し、observer自身がnative workerを起動して、返されたJoinHandleを同closure内で所有・joinする。observer起動失敗ならnativeを起動しない。async createから生存JoinHandleを転送するchannelや、handoff失敗時のdetach経路を作らない。完了cause→counter/live削除→通知の順を維持する。約2thread/connectionの費用は測定対象で、軽量VMと表現しない。

起動失敗はnative close成功や実joinとは別の観測にする。失敗登録のaccountingを内部counterへ分け、closeは未完了登録がなくstartingが終了したことを確認する。既存成功ケースのcounter期待を緩めない。

private constructorへcapacity、Configへfilesystem path、fixtureへworker別barrier/failure seamを追加する。ordinalはfixture識別用で、公開IDや新pool algorithmにしない。Noneのseamは従来の適用を維持する。fixtureはexclusive作成に成功したdirectoryだけを所有し、全worker終了後に片付ける。PID/時刻だけの一意性には依存しない。

## 不採用

- live-emptyを数値だけ変えて終了観測を直さない案。
- timer polling、単一global直列化、Tokio join taskへの丸投げ。
- native起動後にobserver生成を試し、失敗するとJoinHandleを失う案。
- healthy Aをclose/killして試験を通す、sleepだけで順序を決める、既存assertを削る案。
- public registryや取得期限を先行公開すること。

## 検証の順序

1. この設計とADR010の参照をcommitする。
2. tests-only: cap2並列取得、B startup取消→join→A終端前のC取得、stock detach先行permit、B terminal failureの独立公開、close競合、observer起動失敗を追加する。独立レビュー後、native起動失敗も別の回帰にする。最初の未実装compile failureと、その後の実契約違反を分ける。
3. 容量/fixtureだけを接続した中間実装で、単一reaperのhead-of-line回帰が実際に失敗することを観測し、sourceとログを残す。そのsourceを公開完成版とはしない。
4. 独立observerを実装し、新しい回帰と元43件、runtime単独/全workspace、fmt/clippy、既存fuzz、4 OS CIを確認する。CIは既存sqlite_prototype stepを使う。
5. native join/closeと容量の条件をbarrierで観測する。新fixtureの巨大allocation、SQL write並列性能、公開Pool APIを保証しない。必要なcostは同条件で観測し、#82の別source測定を新実装へ流用しない。

## 残る判断と次の作業

取得期限の予算受渡しと0ms、巨大capacityのstock allocation、公開Failure/Parameters/Options、capture/registry/sealed SQLは[次の縦切り](sqlite-public-slice-plan.md)に残る。予約後のBEGIN/busyをacquire期限へ含めず、段階ごとに予算をリセットしない。新policy、任意上限、依存変更、unsafe、一般region/effectが必要なら根拠と代替案を提示する。Phase 5はPhase 4のacceptance後。
