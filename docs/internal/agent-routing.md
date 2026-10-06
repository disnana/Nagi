# Codex agent構成と検証

2026-10-06。[AGENTS.md](../../AGENTS.md)が起動判断、昇格、独立review、並列数の方針を持ち、[config](../../.codex/config.toml)と[role設定](../../.codex/agents/fast_worker.toml)が実設定を持つ。通常は親を含め3体まで（子thread上限2）。同じmodel/reasoningは同時1体、Sol Highの実装と通常reviewは順番に行う。Astraはeffort全体で同時1体。設定ファイルを追加しただけでモデルの実行可否や予算制限を保証したとは扱わない。

| role | model ID | effort |
|---|---|---|
| Fast Worker | gpt-6-luna | max |
| Deep Engineer | gpt-6.1-sol | high |
| Independent Reviewer | gpt-6.1-sol | high |
| Architect | gpt-6.1-sol | xhigh |
| Critical Reviewer | gpt-6.1-sol | max |
| Astra Final Verifier | gpt-6-astra | max |

Codex upstream `9479e1fdb7396c3e3254f5e123489af829186893`の[discovery](https://github.com/openai/codex/blob/9479e1fdb7396c3e3254f5e123489af829186893/codex-rs/agent-roles/src/discovery.rs)・[loader](https://github.com/openai/codex/blob/9479e1fdb7396c3e3254f5e123489af829186893/codex-rs/agent-roles/src/loader.rs)・[role schema](https://github.com/openai/codex/blob/9479e1fdb7396c3e3254f5e123489af829186893/codex-rs/agent-roles/src/agent_role_config.rs)・[config schema](https://github.com/openai/codex/blob/9479e1fdb7396c3e3254f5e123489af829186893/codex-rs/config/src/config_toml.rs)を確認した。`.codex/agents/*.toml`は自動検出され、各roleにname、description、developer_instructionsが必要。別登録表は不要。max_concurrent_threads_per_sessionはspawnされた子threadを数える。max_depthはV1用でV2では無視されるため、AGENTSでも子からの追加spawnを禁止する。

TOML全7件の型・必須項目・未知key・model/effort・name重複・pathと方針の一致を検査した。CLIは`codex-cli 0.159.0-alpha.3`。そのbundled catalogにはLuna/Astraとgpt-6-solがあり、gpt-6.1-solはなかった。一方、このworkspaceのspawn APIはgpt-6.1-solを提供し、Sol Highの独立reviewerを実際に起動した。両IDが同じモデルとは判断せず、指定された6.1の設定を保持した。

standalone CLIの`exec --strict-config`、role読み込みprobe、6.1の実行probeはread-only runtime stateの初期化で停止した。CLIでの実読み込み・モデル実行成功は未確認。`debug models`の成功はcatalog表示だけで、role読み込みの証拠ではない。別環境で使う際はモデルcatalogとrole loaderを確認し、未対応なら親へ根拠を返す。別モデルへの暗黙fallbackはしない。

検証原ログはこのセッションの`/tmp/nagi-agent-routing/`に保存した。TOML/spec照合はWorkerと親の読戻しで確認し、Task compiler/runtime変更とは別の差分として扱う。
