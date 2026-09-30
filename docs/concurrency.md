# 並行処理とscheduler

TokioのM:N executorを使用します。Nagiの独自schedulerは未実装です。HTTP、timer、channelはasync待機し、SQLiteは専用native threadへ渡します。CPU処理は上限4のoffloadへ渡します。

通常のmutable stateを複数taskへ暗黙に共有しません。所有値のmove、immutable shared、bounded channel、actor内部のstateを基本にします。最終的なSend判定もRust backendが担当します。

```python
async def main() -> Result[unit, Error]:
    result = try await cpu_sum(100000)
    print(result)
    return ok(print("完了"))
```

CPU offloadはevent loopを塞ぎにくくしますが、開始したspawn_blockingの処理を外側のキャンセルだけで停止できません。長いCPU kernelに協調的キャンセルと予算を追加する必要があります。

大量task試験では全taskの最初のpollを確認し、gateを解放するまで完了を止めます。全件待機時のRSSと、解放後の全join・未完了0件を記録します。生成したtask件数だけでは同時待機の証明になりません。接続数試験も別に扱います。
