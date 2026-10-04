# コードを図にする

`nagic map`は、型、モジュール依存、関数呼び出しを静的に調べます。Rustのビルドやアプリの起動は行いません。Nagi 0.1.8から使えます。

## 出力する

[小さなサンプル](../examples/code-map/)を、リポジトリのルートから調べます。

```sh
nagic map types --project examples/code-map
nagic map modules --project examples/code-map --format d2
nagic map calls --project examples/code-map --format html --output calls.html
```

種類を省くと`types`、形式を省くとMermaidです。`--output`を省くと標準出力へ書きます。HTMLは単一ファイルで、ブラウザーからノードやソース位置、関係を確認できます。

| 種類 | 調べるもの |
| --- | --- |
| `types` | class・enum、フィールド、引数・戻り値にある型関係。`shared`・`view`・`owned`・Result・nullableを区別する |
| `modules` | 読み込んだファイルと標準モジュールの依存関係 |
| `calls` | 静的に解決できた関数呼び出しとRust FFIへの境界 |

## 図を絞る

```sh
nagic map types --project examples/code-map --focus Post --depth 1
nagic map calls --project examples/code-map --module service
```

`--focus`は型や関数の名前です。同名の候補が複数あれば、表示される完全な名前かIDを指定します。`--depth`はfocusから関係をたどる回数で、0はそのノードだけ、既定は1です。現在は`--depth`を`--focus`と組み合わせます。

`--module`はimportの別名、標準モジュール名、読み込んだファイルの完全なパスで選べます。選んだモジュール内に絞ってからfocusとdepthを適用します。

## D2で描画する

MermaidとD2のテキスト出力には、追加ツールは不要です。D2の図はモジュールごとにまとめ、依存や呼び出しの方向を表示します。

SVG・PNGには、PATHに[D2](https://d2lang.com/)が必要です。出力ファイルには形式に合う拡張子を付けます。既定のレイアウトはELKです。Dagre・TALAも指定できますが、実際の描画はインストールしたD2とそのレイアウトエンジンに依存します。

```sh
nagic map types --project examples/code-map --format svg --output types.svg
nagic map modules --project examples/code-map --format png --output modules.png --layout tala
```

## 解析の範囲

内部では、検査済みのコードを共通のGraph IRへ変換してから描画します。`--format json`はschema version、ノード、意味付きの辺、グループ、ソース位置を出力します。Rendererの変更は型検査や実行コードに影響しません。

関数引数として登録するcallbackやローカル関数値の呼び出しは、現在は辺として表示しません。Rust関数の内部、HTTPの実行時フロー、Actorの実行時構成も解析しません。未解決の関係は警告で示します。警告は入力全体のもので、focusやmoduleで絞っても残ります。`trace`、Graph IRを使う`cost`、D2でのアーキテクチャ図やローカルWebサーバーは今後の範囲です。既存の`--cost-report`は現在の形式で利用できます。
