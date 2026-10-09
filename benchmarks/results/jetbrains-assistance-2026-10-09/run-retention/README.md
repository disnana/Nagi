# IDE一時run世代の回収検証

production/test sourceは `3a044fb4e93a56fb3de93afbc7b8f05ad6306a1a`。後続は公開案内・結果記録のみ。sourceと原ログのSHA-256は[provenance.json](provenance.json)を参照。

Linux native48件、Java70件（failure/skip0）、workspace exit0（raw101 result blocks・1039 pass・failed0・既存ignored1）が成功。件数は子process再実行を含むraw集計でunique test数ではない。warm cache・debug0・jobs2でありclean buildではない。compiler all-target clippyも成功。

`initial-red`とreview REDはnative run/buildを完了した後の保持assertion失敗。parser rejectionやcompiler panicをGREENへ読み替えていない。path入力の初回private期待はforeign-out namespaceを同一out refsへ混ぜない設計へ修正し、同out path rootと別out exportのnative保護へ分けた。Cargoの任意source layoutを解析するためにsrc/lib.rsを推測する保証は追加しない。Windows by-handle MetadataExt IDのnightly依存は除去し、stableのnamespace参照・native bytes oracleを維持した。

独立Sol Highの5件はsource修正済み。native待機後X拒否、親終了後TLS/OS exitまでのlease、direct起動、partial/tree-gone X journal再開、High/保存Low/手書きLow、concurrent current-input/last-goodを観測。async fixtureはentry-before-block_onだけのminimal runtimeで、一般Tokio意味論の証明ではない。Windows junctionはcfg登録済み・Linux未実行。最終HEADの4 OS/4 IDE/Plugin VerifierはPR #104で確認する。過去feafba8のCIを今回の成功へ数えない。

通常CLI成果物、別outへexportしたimmutable入力、未知metadata、links/reparse、failed staging、共有Cargo cacheは保護する。世代数・総容量の厳密な上限、任意Rust/include依存の網羅、電源断・普遍fsync/OOM回復は保証しない。GUIはユーザーによるHigh run/exit0/context Run確認の範囲で、候補の補完/navigation/live diagnostics/Low/Stop/trustの全GUI検証は未確認。

## Docs / distribution readback

統合後のcurrent Docs/evidence checksはRust/CargoやGradle cacheを起動せず実施した。website buildは102 pages、local links/anchors/assetsを検証。CI policy unittest 63件とrelease unittest 116件が最終実行で成功した。初回policy実行はDocs内GitHub Releases URLが既存contract testに必要なのに文面修正で外れたため2 assertion failureとなった。generic release linkを戻して再実行し63/63 GREEN、test期待やmute条件は変えていない。初回の[失敗ログ](ci-policy.log)と[修正後pass](ci-policy-pass.log)を両方保持する。

[website build log](site-build.log)、[policy log](ci-policy-pass.log)、[release log](release-unit.log)。Marketplace readbackのraw evidenceは `/workspace/nagi-run-retention-artifacts/40-marketplace-update-1189900-targeted-readback.json`、GitHub/Marketplace distribution readbackは `/workspace/nagi-run-retention-artifacts/44-github-marketplace-distribution-comparison.json` に保存した。GitHub `jetbrains-v0.1.2` public releaseのcommon ZIPと91-byte SHA-256 sidecarはHTTP 200で、Marketplace public list/feedが0.1.1のみを返した事実はowner approval workflowの否定ではない。案内Docsはexisting numeric listing IDを維持し、Versionsで選べる版とIDE targetを確認する表現にした。

ユーザー画面で確認できるのはHigh Runの出力/exit0、上部Nagi Run構成、右クリック標準Run actionの表示まで。実クリックとの因果、exact IDE/compiler source identity、補完/navigation/live diagnostics/Low/Stop/trust/project switch GUIは未確認である。CI fixtureの成功をGUI PASSへ読み替えない。
