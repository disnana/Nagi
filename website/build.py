"""Build a static introduction and Docs site from the repository's Markdown."""
from __future__ import annotations

import argparse
import html
import re
import shutil
from collections import Counter
from html.parser import HTMLParser
from pathlib import Path
from urllib.parse import quote, unquote, urlsplit
from xml.sax.saxutils import escape

from markdown_it import MarkdownIt
from pygments import highlight
from pygments.formatters import HtmlFormatter
from pygments.lexer import RegexLexer, bygroups
from pygments.lexers import TextLexer, get_lexer_by_name
from pygments.style import Style
from pygments.token import Comment, Error, Generic, Keyword, Name, Number, Operator, Punctuation, String, Text
from pygments.util import ClassNotFound

ROOT = Path(__file__).resolve().parents[1]
HERE = Path(__file__).resolve().parent
GROUPS = [
    ("まず読む", [("getting-started", "準備と最初の実行"), ("language-guide", "コードを書きながら学ぶ"), ("editor", "エディターの操作例"), ("syntax", "文法の早見表"), ("builtins", "よく使う関数")]),
    ("言語の基本", [("types", "型と推論"), ("classes", "class"), ("ownership", "所有権"), ("view-and-zero-copy", "viewとコピー"), ("error-handling", "エラー処理")]),
    ("アプリを作る", [("http", "HTTPとHTML"), ("json", "JSON"), ("database", "SQLite"), ("modules-and-rust", "importとRust連携"), ("projects", "プロジェクト設定"), ("async", "asyncとscope"), ("concurrency", "並行処理")]),
    ("サンプル", [("web-demo", "タスク管理デモ"), ("result-api", "Result APIサンプル")]),
    ("設計と開発", [("introduction", "目的と実装範囲"), ("low-language", "HighとLow"), ("memory-model", "メモリモデル"), ("compiler-internals", "コンパイラの構成"), ("actor", "actor"), ("supervisor", "Supervisor"), ("queue", "queue"), ("ffi", "FFI"), ("performance", "性能の読み方"), ("measurements", "測定結果"), ("roadmap", "今後の開発"), ("vscode-extension", "VS Code拡張の設定")]),
]
SOURCES = {ROOT / "docs/README.md": "docs/"}
EXTRA = {
    "web-demo": ROOT / "test-nagi-code/web-demo/README.md",
    "result-api": ROOT / "test-nagi-code/result-api/README.md",
    "measurements": ROOT / "PERFORMANCE.md",
    "vscode-extension": ROOT / "editors/vscode-nagi/README.md",
}
for _, entries in GROUPS:
    for slug, _ in entries:
        SOURCES[EXTRA.get(slug, ROOT / f"docs/{slug}.md")] = f"docs/{slug}/"


class CodeStyle(Style):
    background_color = "#f5f8fa"
    styles = {
        Text: "#1d2937", Comment: "italic #526176", Keyword: "bold #196233",
        Keyword.Type: "#82351f", Name: "#1d2937", Name.Function: "#213f85",
        String: "#315a86", Number: "#246e4d", Operator: "#526176",
        Punctuation: "#526176", Generic: "#526176", Error: "#9b2828",
    }


class NagiLexer(RegexLexer):
    name = "Nagi"
    tokens = {"root": [
        (r"\s+", Text), (r"#.*$", Comment.Single),
        (r'(?:"(?:\\.|[^"\\])*"|\x27(?:\\.|[^\x27\\])*\x27)', String),
        (r"\b(def|fn|class|record)(\s+)([A-Za-z_]\w*)", bygroups(Keyword, Text, Name.Function)),
        (r"\b(async|await|try|return|if|else|while|for|in|match|case|let|import|scope|with|spawn|extern)\b", Keyword),
        (r"\b(True|False|true|false|None|null)\b", Keyword.Constant),
        (r"\b(i\d+|u\d+|f\d+|bool|str|bytes|unit|List|Result|Error|Db|Html|view|shared)\b", Keyword.Type),
        (r"\b(print|len|range|ok|error|some|copy|parse_i64)\b", Name.Builtin),
        (r"\b\d+(?:\.\d+)?\b", Number), (r"@[A-Za-z_]\w*", Name.Decorator),
        (r"[+*/=<>!%-]+", Operator), (r"[():,;\[\]{}.]", Punctuation),
        (r"[A-Za-z_]\w*", Name), (r".", Text),
    ]}


def code_block(source: str, language: str) -> str:
    try:
        lexer = NagiLexer() if language in {"nagi", "low"} else get_lexer_by_name(language)
    except ClassNotFound:
        lexer = TextLexer()
    code = highlight(source, lexer, HtmlFormatter(nowrap=True))
    button = '<button class="copy-button" type="button" data-copy aria-label="このコードをコピー">コピー</button>'
    return f'<div class="code-block">{button}<pre class="highlight" tabindex="0" aria-label="コード"><code>{code}</code></pre></div>\n'


def slugify(text: str) -> str:
    return re.sub(r"\s+", "-", re.sub(r"[^\w\- ]", "", text.lower())).strip("-") or "section"


class Links(HTMLParser):
    def __init__(self):
        super().__init__()
        self.ids: list[str] = []
        self.urls: list[str] = []

    def handle_starttag(self, tag, attrs):
        attributes = dict(attrs)
        if attributes.get("id"):
            self.ids.append(attributes["id"])
        if tag in {"a", "link", "img", "script"}:
            url = attributes.get("href") or attributes.get("src")
            if url:
                self.urls.append(url)


def check_links(output: Path, base: str) -> int:
    pages = {}
    for path in output.rglob("*.html"):
        parser = Links()
        parser.feed(path.read_text(encoding="utf-8"))
        assert len(parser.ids) == len(set(parser.ids)), f"Duplicate ID in {path}"
        pages[path.resolve()] = parser
    errors = []
    for page, parser in pages.items():
        for url in parser.urls:
            parsed = urlsplit(url)
            if parsed.scheme or parsed.netloc:
                continue
            if parsed.path:
                if not parsed.path.startswith(base):
                    errors.append(f"{page.name}: URL outside base path: {url}")
                    continue
                target = output / unquote(parsed.path[len(base):])
                if parsed.path.endswith("/"):
                    target /= "index.html"
            else:
                target = page
            if not target.is_file():
                errors.append(f"{page.relative_to(output)}: missing target: {url}")
            elif parsed.fragment and target.resolve() in pages and unquote(parsed.fragment) not in pages[target.resolve()].ids:
                errors.append(f"{page.relative_to(output)}: missing anchor: {url}")
    if errors:
        raise ValueError("\n".join(errors))
    return len(pages)


def build(output: Path, base: str, origin: str, repo: str, ref: str) -> None:
    output = output.resolve()
    if output == ROOT / "build" or not output.is_relative_to(ROOT / "build"):
        raise ValueError("Output must be a subdirectory of build/")
    if output.exists():
        shutil.rmtree(output)
    output.mkdir(parents=True)
    shutil.copytree(HERE / "assets", output / "assets")
    css = HtmlFormatter(style=CodeStyle).get_style_defs(".highlight")
    (output / "assets/highlight.css").write_text(css, encoding="utf-8")
    template = (HERE / "templates/page.html").read_text(encoding="utf-8")

    def page(route, title, description, layout, body_class=""):
        values = dict(base=base, repo=html.escape(repo, quote=True), ref=quote(ref, safe=""),
                      title=html.escape(title), description=html.escape(description, quote=True),
                      canonical=html.escape(origin + base + route, quote=True), layout=layout, body_class=body_class)
        rendered = template
        for key, value in values.items():
            rendered = rendered.replace("{{" + key + "}}", value)
        if re.search(r"\{\{[a-z_]+\}\}", rendered):
            raise ValueError(f"Unresolved template value: {route}")
        destination = output / route / "index.html" if not route.endswith(".html") else output / route
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_text(rendered, encoding="utf-8")

    def navigation(current):
        sections = [f'<a class="docs-nav-title" href="{base}docs/">Nagi Docs</a>']
        for title, entries in GROUPS:
            links = []
            for slug, label in entries:
                active = ' aria-current="page"' if current == f"docs/{slug}/" else ""
                links.append(f'<li><a href="{base}docs/{slug}/"{active}>{label}</a></li>')
            sections.append(f'<section><h2>{title}</h2><ul>{"".join(links)}</ul></section>')
        return '<nav class="docs-nav" aria-label="Docsの目次">' + "".join(sections) + "</nav>"

    for source, route in SOURCES.items():
        if not source.is_file():
            raise ValueError(f"Missing Docs source: {source.relative_to(ROOT)}")
        md = MarkdownIt("commonmark", {"html": False}).enable("table").enable("strikethrough")
        md.renderer.rules["fence"] = lambda tokens, index, options, env: code_block(tokens[index].content, tokens[index].info.split()[0] if tokens[index].info else "text")
        md.renderer.rules["table_open"] = lambda *args: '<div class="table-scroll" tabindex="0" role="region" aria-label="表（横にスクロールできます）"><table>\n'
        md.renderer.rules["table_close"] = lambda *args: '</table></div>\n'

        def link_open(tokens, index, options, env):
            token = tokens[index]
            href = token.attrGet("href") or ""
            parsed = urlsplit(href)
            if parsed.path and not parsed.scheme and not parsed.netloc:
                target = (source.parent / unquote(parsed.path)).resolve()
                if not target.is_relative_to(ROOT) or not target.exists():
                    raise ValueError(f"Invalid source link in {source.relative_to(ROOT)}: {href}")
                if target in SOURCES:
                    url = base + SOURCES[target]
                else:
                    kind = "tree" if target.is_dir() else "blob"
                    url = f"{repo}/{kind}/{quote(ref, safe='')}/{quote(target.relative_to(ROOT).as_posix())}"
                if parsed.fragment:
                    url += "#" + parsed.fragment
                token.attrSet("href", url)
            return md.renderer.renderToken(tokens, index, options, env)

        md.renderer.rules["link_open"] = link_open
        tokens = md.parse(source.read_text(encoding="utf-8"))
        headings = []
        titles = []
        slugs = Counter()
        for index, token in enumerate(tokens):
            if token.type == "heading_open":
                title = tokens[index + 1].content.replace("`", "")
                slug = slugify(title)
                count = slugs[slug]
                slugs[slug] += 1
                if count:
                    slug += f"-{count}"
                token.attrSet("id", slug)
                if token.tag == "h1":
                    titles.append(title)
                elif token.tag == "h2":
                    headings.append((slug, title))
        if len(titles) != 1:
            raise ValueError(f"Expected one page title: {source.relative_to(ROOT)}")
        body = md.renderer.render(tokens, md.options, {})
        outline = "".join(f'<li><a href="#{quote(slug)}">{html.escape(title)}</a></li>' for slug, title in headings)
        sidebar = navigation(route)
        relative = quote(source.relative_to(ROOT).as_posix())
        meta = f'<div class="doc-meta"><span>Nagi 0.1のドキュメント</span><a href="{repo}/blob/{quote(ref, safe="")}/{relative}">このページのソース</a></div>'
        layout = f'<div class="docs-shell"><div class="docs-layout">{sidebar}<div class="docs-content"><details class="mobile-doc-nav"><summary>Docsの目次</summary>{sidebar}</details><main id="main" class="docs-main">{body}{meta}</main></div><nav class="outline" aria-label="このページの目次"><p>このページ</p><ul>{outline}</ul></nav></div></div>'
        page(route, f"{titles[0]} | Nagi Docs", f"Nagiの日本語ドキュメント。{titles[0]}について説明します。", layout, "docs-page")

    example = (HERE / "examples/double.nagi").read_text(encoding="utf-8")
    home = (HERE / "templates/home.html").read_text(encoding="utf-8").replace("{{example}}", code_block(example, "nagi"))
    home = home.replace("{{base}}", base).replace("{{repo}}", html.escape(repo, quote=True))
    page("", "Nagi — 読みやすいコードを、実行ファイルに。", "Nagiは、コードの読みやすさと実行時の効率を大切にしている開発中のプログラミング言語です。日本語の入門ガイドとDocsを用意しています。", home)
    page("404.html", "ページが見つかりません | Nagi", "ページが見つかりません。", f'<main id="main" class="landing section"><h1>ページが見つかりません</h1><p>リンク先が変わったか、URLが間違っているようです。</p><p><a href="{base}docs/">Docsの目次へ</a></p></main>')
    (output / ".nojekyll").touch()
    locations = [origin + base] + [origin + base + route for route in SOURCES.values()]
    sitemap = '<?xml version="1.0" encoding="UTF-8"?><urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">' + "".join(f"<url><loc>{escape(url)}</loc></url>" for url in locations) + "</urlset>"
    (output / "sitemap.xml").write_text(sitemap, encoding="utf-8")
    pages = check_links(output, base)
    print(f"Built {pages} pages; all local links, anchors and assets verified: {output}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, default=ROOT / "build/website")
    parser.add_argument("--base-path", default="/Nagi/")
    parser.add_argument("--site-url", default="https://disnana.github.io")
    parser.add_argument("--repository", default="https://github.com/disnana/Nagi")
    parser.add_argument("--ref", default="main")
    args = parser.parse_args()
    base = "/" + args.base_path.strip("/") + "/" if args.base_path.strip("/") else "/"
    if re.search(r"[^A-Za-z0-9_./-]", base) or any(p in {".", ".."} for p in base.split("/")):
        parser.error("base-path must be a URL path without . or .. segments")
    if urlsplit(args.site_url).scheme != "https" or urlsplit(args.repository).scheme != "https":
        parser.error("site-url and repository must use HTTPS")
    build(args.out, base, args.site_url.rstrip("/"), args.repository.rstrip("/"), args.ref)


if __name__ == "__main__":
    main()
