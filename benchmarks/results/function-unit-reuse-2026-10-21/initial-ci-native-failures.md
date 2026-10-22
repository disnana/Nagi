# 初回CIで観測した既存native検査の失敗

対象HEAD `c9c923d`。compilerの分割flagはこれらのruntime Cargo testでは使用せず、baseからruntime/Cargo.lockの差分はない。原因はまだ特定していない。期限・assert・runtimeを先行変更しない。

- Windows: PR run [39606221471](https://github.com/disnana/Nagi/actions/runs/39606221471)、job119243221501と、release-level run [39606295076](https://github.com/disnana/Nagi/actions/runs/39606295076)、job119245976194で、どちらも`prelogin-issuer state=test_failed` / `source=owner line=15 column=33`。owner期限10秒だが、TTLとSQLite acquire/busyは各1秒であり、時間だけから原因を除外できない。open時点で同じpoolのDDL/commitは成功しているので、初回worker coldと断定しない。
- Linux release-level job119245975793: session_csrf_public_apiの3件が初期TokenReplyで504。補助pool-readiness実験は別実験として成功し、失敗3件のphaseやWindowsの原因を説明する証拠にはしない。

Sol 6.1 Highの事前reviewに従い、次の1回の新HEAD CI観測へPython限定の分類診断を追加する。既存の単一FAILED test・既知位置・入力上限を検査し、完全一致するPreLoginFailureの固定kind/outcomeだけを出す。未知値・重複・余計なpayloadはunavailable、失敗終了コードは1を維持する。秘密や一般panic/SQL/pathを出さず、期限や成功条件を変更しない。分類不能やUnknown outcome等で整合性判断が残れば、根拠なくruntimeを修正せずArchitectへ上げる。

```text
2026-10-21T13:30:52.5577745Z b1_bootstrap_resources cpu=4 load_supported=1 load1_milli=2575 load5_milli=2005 load15_milli=941 memory_supported=1 mem_total_kib=16367524 mem_available_kib=15128664 swap_free_kib=3145724 dirty_kib=814312 writeback_kib=0 disk_supported=1 disk_total_bytes=154894188544 disk_free_bytes=85777027072
2026-10-21T13:30:58.8511004Z safe_reply_diag check=token_response status=504 class=deadline set_cookie_count=0 body_bytes=15
2026-10-21T13:30:58.8512554Z csrf_public_assertion_diag phase=TokenReply status=504 body_bytes=15 set_cookie_count=0
2026-10-21T13:30:58.8514406Z safe_reply_diag check=token_response status=504 class=deadline set_cookie_count=0 body_bytes=15
2026-10-21T13:30:58.8515838Z csrf_public_assertion_diag phase=TokenReply status=504 body_bytes=15 set_cookie_count=0
2026-10-21T13:30:58.8517273Z safe_reply_diag check=token_response status=504 class=deadline set_cookie_count=0 body_bytes=15
2026-10-21T13:30:58.8519179Z csrf_public_assertion_diag phase=TokenReply status=504 body_bytes=15 set_cookie_count=0
2026-10-21T13:31:00.0471438Z b1_supplement_record arm=2 b1_startup_bootstrap stage=issue status=200 created=1 idle=1 elapsed_us=3949
2026-10-21T13:31:00.0472180Z b1_supplement_record arm=2 b1_startup_bootstrap stage=csrf status=200 created=1 idle=1 elapsed_us=7390
2026-10-21T13:31:00.0472814Z b1_supplement_record arm=1 b1_startup_bootstrap stage=issue status=200 created=1 idle=1 elapsed_us=3949
2026-10-21T13:31:00.0473413Z b1_supplement_record arm=1 b1_startup_bootstrap stage=csrf status=200 created=1 idle=1 elapsed_us=7618
2026-10-21T13:31:00.0474658Z b1_supplement_record arm=1 csrf_guarded_deadline_diag status=504 phase=10 event=1 db=1 exit=0 polls=1 begins=0 commands=0 commits=0 budget_ms=1000 entry_remaining_ms=999 phase_remaining_ms=999 exit_remaining_ms=0 selected_deadline_passed=1 terminal=1
2026-10-21T13:31:00.0476193Z b1_supplement_record arm=1 session_native_call_diag status=504 origin=0 terminal=0 diagnostic_valid=0 native_call_state=0 generation=0 call_enter_us=60000001 call_return_us=60000001 native_state=0 commit_result=0 settled_us=60000001 begins=0 commits=0 selected_deadline_passed=0
2026-10-21T13:31:00.0477417Z b1_supplement_result cargo_rc=0 capture_rc=0 captured_bytes=2251 overflow=0 malformed=0 records=6 inventory_ok=1 summary_ok=1 bootstrap_ok=1 diagnostic_available=1
```
