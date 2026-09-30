# 目的と実装範囲

Nagiの狙いは、HTTP、JSON、DB、taskの境界で、同じデータを何度も別の汎用オブジェクトへ作り直すコストを減らすことです。アプリ開発者が通常使うHighと、生成物を調整するLowを分けます。

Highの字下げ構文は読みやすさのためです。動的なPythonオブジェクトやPythonの互換動作は採用していません。Lowは波括弧と明示的な型・宣言を使い、Highからの生成ソースにも手書きにも同じparserとcheckerを適用します。

0.1で動く経路は、High → Lowテキスト → Low AST → 型検査 → Rust → ネイティブです。HTTP、JSON、SQLiteを通るCRUD API、CPUループ、task、actor間通信、workerのpanicと再起動まで実行します。設計のみの機能は各ページで明記します。

独自性を検証する中心は、読み返せるLow、型を維持する関数置換、コストが見えるview/所有値、typed modelへ直接接続するバックエンド経路です。現在の実装だけで、新しいメモリ管理やBEAM級の障害分離を実現したとは判断できません。

書き始める場合は[ドキュメントの目次](README.md)から、[準備と最初の実行](getting-started.md)、[コードを書きながら学ぶ](language-guide.md)へ進んでください。書式を調べるには[文法の早見表](syntax.md)、実測は[性能の測定結果](../PERFORMANCE.md)を参照してください。
