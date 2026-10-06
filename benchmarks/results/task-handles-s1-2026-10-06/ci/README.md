# S1接続sourceのCI読戻し

検証source: `34ac4d585084877372965e3d58ed5c2002604529`、tree: `781e031eafbdc0efa00038007d41cf16dc538862`。

[Nagi checks 37489343115](https://github.com/disnana/Nagi/actions/runs/37489343115)と[website 37489342523](https://github.com/disnana/Nagi/actions/runs/37489342523)は成功。run metadataのhead_shaとPR #88の実headを読み戻した。`source-34ac4d5/` にdecoded job logs、run/job JSON、Task件数の照合結果を保存した。Stage 1のCIは今回へ数えていない。

4 OSのnative5群・runtime17群・公開API3・doc9・checker90/90をそれぞれ原ログで確認。Task native5群には全19 positive対のHigh・保存Low・手書きLow、独立lifecycleと固定seed生成経路を含む。checker90件はnative実行件数ではない。フィルタで0件になった付随suiteを成功件数へ加えない。Linux全workspace原ログは95 result block・933成功・failed0・費用用ignored1（image_child子process再実行3件を含む）。費用用ignoredはローカルで別途明示実行した。

全package、両JetBrains IDE、VS Code、Linux必須step、merge gateは成功。websiteは92ページのbuild/link検査成功。PRのwebsite deployとrelease publishはskipであり、公開やreleaseを行った記録ではない。

この追記は文書/artifactのみ。production 151 source hashesはprovenance.jsonと一致する。追記headのChecksはPR #88で別に読み戻す。CIの成功を任意Rust Drop/panic回復、non-yielding強制停止、外部副作用rollbackの保証へ広げない。
