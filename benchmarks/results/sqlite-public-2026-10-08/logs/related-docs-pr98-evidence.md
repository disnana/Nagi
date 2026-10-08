# 関連Docs PR #98のCI証拠

この記録はonboarding Docs PR #98だけに適用する。PR #98のheadはbc6a76b、main baseは6765767。[checks run 37701060949](https://github.com/disnana/Nagi/actions/runs/37701060949)と[website run 37701060552](https://github.com/disnana/Nagi/actions/runs/37701060552)が成功し、4 OS/Linux/IDE/package/gate/website結果を読み戻した後、PRはreadyになった。未merge。これらはPR #99のCI結果ではない。

各4 OS raw jobで同じharnessが成功した。完全なraw logsはコピーせず、line excerptとSHA-256を残す。

| raw log basename | excerpt | SHA-256 |
|---|---|---|
| ci-113064594902.log | Onboarding: bilingual code matches; High/saved Low check and build; 10 native cases passed | a26474acdce83342f87e1b4ccf802f2b750ee74c6c096472c5045951a0253848 |
| ci-113064594943.log | Onboarding: bilingual code matches; High/saved Low check and build; 10 native cases passed | 22250ab1ba06445bf73ec3deb943562305608dabc63b171b285d14479d2bae6e |
| ci-113064594981.log | Onboarding: bilingual code matches; High/saved Low check and build; 10 native cases passed | c079344c829ad48bafc7ba6cc13807e32ea7eb0c5b8a9901eff3b0ce5a789fdd |
| ci-113064595011.log | Onboarding: bilingual code matches; High/saved Low check and build; 10 native cases passed | 8b0a09b151bceeca3c02cba042004c9e784ec9073058cab9e1431e4e3663dbd7 |

The originating raw logs remain outside this repo; these four basenames and hashes identify them without adding full CI output. PR #99 needs its own latest-head checks and website result.
