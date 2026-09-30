# Lowと手書き統合

Lowは内部のバイナリIRではなく、独立parserを持つ読み書きできるテキストです。

```text
fn twice(x: i64) -> i64 {
    return x * 2;
}
```

Highから生成したLowには推論済みの型とlet宣言を出します。生成テキストを再解析し、nativeの通常関数と置換関数を統合してから検査します。native関数はHighの名前解決にも使えます。

```text
@replace generated::score
fn optimized_score(x: i64) -> i64 {
    return x * 6;
}
```

対象の有無、引数・戻り値・asyncの一致、重複置換を検査します。置換は完全な関数単位です。生成関数の内部行をpatchする仕組みや、@override/@customは未実装です。生成ファイルそのものへの直接編集を再生成で保持する機能もありません。変更をnativeへ移す運用を使用します。

Resultのmatchは、Highの字下げを次のように波括弧へ置き換えます。これは関数の例です。

```text
fn number_or(text: view[str], fallback: i64) -> i64 {
    match parse_i64(text) {
        case Ok(number) { return number; }
        case Err(_) { return fallback; }
    }
}
```

両方のcaseが必須です。payloadの型・所有権・借用はHighと同じ規則で検査します。[エラー処理](error-handling.md)も参照してください。

0.1のLowは型付き値、view、class/record、関数、分岐、Resultのmatch、ループ、async/scopeを扱います。生pointer、layout/alignment指定、allocation/free、SIMD命令、unsafe、FFIは今後実装する範囲です。RustのコードをLowとして受け取る方式ではありません。
