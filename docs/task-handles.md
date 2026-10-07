# 並行処理の結果を一度受け取る

Task結果handleとSupervisor monitorのS2移行はNagi 0.1.11への導入対象です。S2は既存APIで実装済みで、最終4 OS CIが成功し、mainへ反映しました。公開配布で使えるかは公式Release記録を確認してください。旧`spawn work()`は引き続き使えます。[実行可能なHigh・手書きLowの例](../test-nagi-code/library-examples/task-results/README.md)があります。

```nagi
from std.task import discard

async def answer() -> i64:
    return 42

async def main() -> Result[unit, Error]:
    async with scope:
        task = spawn answer()
        result = await task
        match result:
            case Ok(value):
                print(value)
            case Err(failure):
                print("子taskが故障しました")
        background = spawn sleep(10)
        discard(background)
    return ok(print("完了"))
```

`task = spawn answer()`は並行処理を開始し、`Task[i64]`を得ます。`await task`はhandleを一度消費し、子の実際の終了を待ってから`Result[i64, TaskFailure]`を返します。通常のasync呼出しを直接awaitする書き方も使えます。Futureそのものの保存には対応しません。

Taskは結果の型にかかわらず非Copy・非Clone・非sharedです。作成したscope内のローカルに限り、正常なbinding出口・scope出口・loop継続までにawaitかdiscardが必要です。両方の分岐で義務を満たしてください。`from std.ownership import move`の`alias = move(task)`はhandleと義務を移します。裸の`move(task)`で放棄はできません。

短絡`and`/`or`の右辺は評価が省略されることがあり、値が存在する場合の`env` fallbackは評価されません。その中だけでawait/discardしても、正常出口の義務を満たしません。checkerは条件の定数値でこの義務を省略しないため、先にTaskを受け取り、得た通常のResultを条件付きで使ってください。左辺や`env`のkeyのように必ず評価する位置での受取は有効です。

Taskを関数の引数や戻り値、field、List、Option、Resultなどのwrapper、他taskへ渡すことはできません。内側の別scopeで外側のTaskを受け取ることもできません。内側のscopeが終わってから、元のscopeで受け取ってください。scope内のreturnは引き続き未対応です。

## 業務結果とtask故障

子が`Result[T, E]`を返すと、受取型は`Result[Result[T, E], TaskFailure]`です。内側の業務Errだけでは兄弟を停止しません。panic、予期しない取消、同じscope内の旧spawnのErr、bridgeのprotocol故障は外側の故障になります。

`from std.task import kind, message, TaskFailureKind`で故障を調べられます。`kind(failure)`はCopyなenumで、定数は`Panicked`、`Cancelled`、`LegacyError`、`Internal`です。`message(failure)`はfailureを借用元とする`view[str]`です。借用が残る間、failureをmoveできません。TaskFailureはopaqueな非Copy・非Clone・非shared値で、Errorへの暗黙変換はありません。panic payloadの任意の文字列を保持する保証もありません。

故障を受け取りmatchしても、scopeの故障状態は残ります。最初に観測した故障をprimaryとし、兄弟へ取消を要求し、全子を実joinしてからscope出口のErrorを返します。観測順はspawn順とは限りません。bodyの`try`による元Errは、後から子が故障しても置換しません。

`discard(task)`はunitを返し、結果の受取を放棄します。子の停止、detach、故障の抑制、終了待ちの省略、資源closeの完了を意味しません。scopeはその子の終了も待ちます。Taskを含むscope内に旧`spawn work()`が混在する場合、その旧spawnの`Result[unit, Error]` Errは引き続きscope故障です。Supervisor/HTTPの旧連携も維持しています。

## SupervisorとHTTPの移行

[サービス例](../test-nagi-code/library-examples/supervised-service/README.md)ではmonitorだけをTaskへ移し、HTTPの旧spawnを維持します。`await monitor_task`の`Ok(inner)`を親の`try inner`で処理すると、Supervisorのterminal Errがbody Errとなり、HTTPへ取消を要求して直接の子の実join後に元Errorを返します。単にdiscardすると、この内側ErrはHTTPを止めません。正常なSupervisor shutdownはHTTPを継続します。HTTP故障が先に観測されれば、それがscopeのprimaryになります。

HTTP stateはSupervisor contextを保持しません。外部のshared/native Arcがcontextを保持すると、その解放までSupervisorのcleanupが待つ場合があり、Taskは参照循環を解消しません。

親FutureのDropやpanicでは取消を要求しますが、同期Dropから実join完了は保証できません。scopeの直接の子が終了しても、Rust連携内の任意の子taskやHTTP handlerの非同期close完了を保証するものではありません。yieldしない処理の強制停止や、DBなどの外部副作用の巻戻しも保証しません。[asyncとscope](async.md)、[並行処理](concurrency.md)、[実装と検証状況](internal/task-handles-s1-results.md)も参照してください。
