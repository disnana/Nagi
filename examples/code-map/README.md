# コードマップの例

[English](README.en.md)

`Post`は`User`への共有参照を持ち、`service.nagi`が投稿を作ります。`main.nagi`から両方の関数を呼び、`Nagi`と出力します。

リポジトリのルートで実行します。

```sh
nagic run --project examples/code-map
nagic map types --project examples/code-map --format mermaid
nagic map modules --project examples/code-map --format d2
nagic map calls --project examples/code-map --format html --output calls.html
```

| 図 | D2で描画した例 | Mermaid | D2 |
| --- | --- | --- | --- |
| 型と引数・戻り値 | [types.svg](visuals/types.svg) | [types.mmd](visuals/types.mmd) | [types.d2](visuals/types.d2) |
| importとモジュールの利用 | [modules.svg](visuals/modules.svg) | [modules.mmd](visuals/modules.mmd) | [modules.d2](visuals/modules.d2) |
| 関数呼び出し | [calls.svg](visuals/calls.svg) | [calls.mmd](visuals/calls.mmd) | [calls.d2](visuals/calls.d2) |

SVGはD2 0.9.0のTALAで生成しています。配置はRendererとレイアウトエンジンが決め、Graph IRには型や呼び出しなどの関係を保存します。[形式・絞り込み・解析範囲](../../docs/code-map.md)も参照してください。
