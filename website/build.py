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
DEFAULT_REPOSITORY = "https://github.com/disnana/Nagi"


def custom_domain(repository: str) -> str | None:
    # A fork must not inherit the upstream domain from its copy of CNAME.
    if repository.rstrip("/").lower() != DEFAULT_REPOSITORY.lower():
        return None
    cname = HERE / "CNAME"
    if not cname.is_file():
        return None
    domain = cname.read_text(encoding="utf-8").strip().lower()
    label = r"[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?"
    if len(domain) > 253 or not re.fullmatch(rf"{label}(?:\.{label})+", domain):
        raise ValueError("website/CNAME must contain one DNS hostname")
    return domain


def site_defaults(repository: str) -> tuple[str, str]:
    domain = custom_domain(repository)
    if domain:
        return "/", f"https://{domain}"
    parsed = urlsplit(repository)
    parts = parsed.path.strip("/").split("/")
    if parsed.hostname == "github.com" and len(parts) == 2:
        owner, name = parts
        base = "/" if name.lower() == f"{owner.lower()}.github.io" else f"/{name}/"
        return base, f"https://{owner.lower()}.github.io"
    return "/Nagi/", "https://disnana.github.io"


GROUPS = [
    ("入門", [("getting-started", "準備と最初の実行"), ("language-guide", "コードを書きながら学ぶ"), ("editor", "エディターの操作例")]),
    ("言語リファレンス", [("syntax", "文法の早見表"), ("builtins", "組み込み関数"), ("types", "型と推論"), ("classes", "class"), ("ownership", "所有権"), ("view-and-zero-copy", "viewとコピー"), ("error-handling", "エラー処理")]),
    ("アプリを作る", [("http", "使い方"), ("http-server", "APIリファレンス"), ("http-stdlib-performance", "性能測定"), ("http-legacy", "旧API"), ("json", "JSON"), ("database", "SQLite"), ("modules-and-rust", "importとRust連携"), ("libraries", "自作ライブラリとRustの資産"), ("projects", "プロジェクト設定"), ("async", "asyncとscope"), ("concurrency", "並行処理"), ("actor", "actorの書き方"), ("supervisor", "再起動と停止"), ("actor-reference", "APIリファレンス"), ("actor-performance", "性能測定")]),
    ("サンプル", [("library-examples", "サンプルプロジェクト一覧"), ("web-demo", "タスク管理デモ"), ("result-api", "Result APIサンプル")]),
    ("設計と開発", [("library-design", "ライブラリとRust連携の設計案"), ("introduction", "Nagiについて"), ("low-language", "HighとLow"), ("memory-model", "メモリの扱い"), ("compiler-internals", "コンパイラの構成"), ("code-map", "コードを図にする"), ("queue", "キューの試験"), ("ffi", "他の言語との連携"), ("performance", "性能の読み方"), ("measurements", "測定結果"), ("http-capacity", "通信の負荷試験"), ("roadmap", "今後の開発"), ("vscode-extension", "VS Code拡張の設定")]),
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

ENGLISH_GROUPS = [
    ("First steps", [("getting-started", "Setup and first run"), ("language-guide", "Learn by writing code"), ("editor", "Editor walkthrough")]),
    ("Language reference", [("syntax", "Syntax reference"), ("builtins", "Built-in functions"), ("types", "Types and inference"), ("classes", "Classes"), ("ownership", "Ownership"), ("view-and-zero-copy", "Views and copying"), ("error-handling", "Error handling")]),
    ("Build an application", [("http", "Guide"), ("http-server", "API reference"), ("http-stdlib-performance", "Measurements"), ("http-legacy", "Legacy API"), ("json", "JSON"), ("database", "SQLite"), ("modules-and-rust", "Imports and Rust"), ("libraries", "Libraries and Rust assets"), ("projects", "Project configuration"), ("async", "Async and scopes"), ("concurrency", "Concurrency"), ("actor", "Writing actors"), ("supervisor", "Restart and shutdown"), ("actor-reference", "API reference"), ("actor-performance", "Measurements")]),
    ("Examples", [("library-examples", "Sample projects"), ("web-demo", "Task management demo"), ("result-api", "Result API example")]),
    ("Design and development", [("library-design", "Library design proposal"), ("introduction", "About Nagi"), ("low-language", "High and Low"), ("memory-model", "Memory handling"), ("compiler-internals", "Compiler internals"), ("code-map", "Code maps"), ("queue", "Queue experiments"), ("ffi", "Language interfaces"), ("performance", "Reading benchmarks"), ("measurements", "Measurements"), ("http-capacity", "HTTP load tests"), ("roadmap", "Roadmap"), ("vscode-extension", "VS Code extension settings")]),
]
ENGLISH_SOURCES = {ROOT / "docs/en/README.md": "docs/"}
for _, entries in ENGLISH_GROUPS:
    for slug, _ in entries:
        ENGLISH_SOURCES[ROOT / f"docs/en/{slug}.md"] = f"docs/{slug}/"
if set(SOURCES.values()) != set(ENGLISH_SOURCES.values()):
    raise ValueError("Japanese and English Docs must contain the same pages")

NAV_TOPICS = {
    "http": ("http", "http-server", "http-stdlib-performance", "http-legacy"),
    "actor": ("actor", "supervisor", "actor-reference", "actor-performance"),
}
NAV_TOPIC_LABELS = {
    "ja": {"http": "HTTP", "actor": "actorとSupervisor"},
    "en": {"http": "HTTP", "actor": "Actors and Supervisors"},
}

LABELS = {
    "ja": dict(skip_label="本文へ移動", home_label="Nagi ホーム", menu_label="サイトのメニュー",
               start_label="はじめる", status_label="Nagi 0.1 · 開発中", footer_label="フッター",
               other_language="en", other_label="English", switch_label="Read this page in English",
               docs_contents="Docsの目次", page_contents="このページの目次", on_page="このページ",
               source_label="このページのソース", docs_version="Nagi 0.1 · 開発中のDocs",
               copy_label="コピー", copy_aria="このコードをコピー", code_aria="コード",
               table_aria="表（横にスクロールできます）"),
    "en": dict(skip_label="Skip to content", home_label="Nagi home", menu_label="Site navigation",
               start_label="Get started", status_label="Nagi 0.1 · In development", footer_label="Footer",
               other_language="ja", other_label="日本語", switch_label="このページを日本語で読む",
               docs_contents="Docs contents", page_contents="On this page", on_page="On this page",
               source_label="Page source", docs_version="Nagi 0.1 · Development docs",
               copy_label="Copy", copy_aria="Copy this code", code_aria="Code",
               table_aria="Table (scroll horizontally)"),
}


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
        (r"\b(def|fn|class|record|enum)(\s+)([A-Za-z_]\w*)", bygroups(Keyword, Text, Name.Function)),
        (r"\b(async|await|try|return|if|else|while|for|in|match|case|let|import|from|as|scope|with|spawn|extern)\b", Keyword),
        (r"\b(True|False|true|false|None|null)\b", Keyword.Constant),
        (r"\b(i\d+|u\d+|f\d+|bool|str|bytes|unit|List|Result|Error|Db|Html|view|shared)\b", Keyword.Type),
        (r"\b(print|len|range|ok|error|some|copy|parse_i64)\b", Name.Builtin),
        (r"\b\d+(?:\.\d+)?\b", Number), (r"@[A-Za-z_]\w*", Name.Decorator),
        (r"[+*/=<>!%-]+", Operator), (r"[():,;\[\]{}.]", Punctuation),
        (r"[A-Za-z_]\w*", Name), (r".", Text),
    ]}


def code_block(source: str, language: str, locale: str = "ja") -> str:
    try:
        lexer = NagiLexer() if language in {"nagi", "low"} else get_lexer_by_name(language)
    except ClassNotFound:
        lexer = TextLexer()
    code = highlight(source, lexer, HtmlFormatter(nowrap=True))
    labels = LABELS[locale]
    button = f'<button class="copy-button" type="button" data-copy aria-label="{labels["copy_aria"]}">{labels["copy_label"]}</button>'
    return f'<div class="code-block">{button}<pre class="highlight" tabindex="0" aria-label="{labels["code_aria"]}"><code>{code}</code></pre></div>\n'


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
    domain = custom_domain(repo)
    if output.exists():
        shutil.rmtree(output)
    output.mkdir(parents=True)
    if domain and origin == f"https://{domain}" and base == "/":
        (output / "CNAME").write_text(domain + "\n", encoding="utf-8")
    shutil.copytree(HERE / "assets", output / "assets")
    css = HtmlFormatter(style=CodeStyle).get_style_defs(".highlight")
    (output / "assets/highlight.css").write_text(css, encoding="utf-8")
    template = (HERE / "templates/page.html").read_text(encoding="utf-8")

    def page(route, title, description, layout, locale, body_class=""):
        prefix = "en/" if locale == "en" else ""
        other_prefix = "" if locale == "en" else "en/"
        values = dict(base=base, repo=html.escape(repo, quote=True), ref=quote(ref, safe=""),
                      title=html.escape(title), description=html.escape(description, quote=True),
                      canonical=html.escape(origin + base + prefix + route, quote=True), layout=layout, body_class=body_class,
                      language=locale, language_base=base + prefix, other_route=base + other_prefix + route,
                      japanese_url=html.escape(origin + base + route, quote=True),
                      english_url=html.escape(origin + base + "en/" + route, quote=True), **LABELS[locale])
        rendered = template
        for key, value in values.items():
            rendered = rendered.replace("{{" + key + "}}", value)
        if re.search(r"\{\{[a-z_]+\}\}", rendered):
            raise ValueError(f"Unresolved template value: {route}")
        destination = output / prefix / route / "index.html" if not route.endswith(".html") else output / prefix / route
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_text(rendered, encoding="utf-8")

    def navigation(current, locale):
        language_base = base + ("en/" if locale == "en" else "")
        title_active = ' aria-current="page"' if current == "docs/" else ""
        sections = [f'<a class="docs-nav-title" href="{language_base}docs/"{title_active}>Nagi Docs</a>']
        topic_pages = {slug for entries in NAV_TOPICS.values() for slug in entries}

        def link(slug, label):
            active = ' aria-current="page"' if current == f"docs/{slug}/" else ""
            return f'<li><a href="{language_base}docs/{slug}/"{active}>{label}</a></li>'

        for index, (title, entries) in enumerate(ENGLISH_GROUPS if locale == "en" else GROUPS):
            links = []
            page_labels = dict(entries)
            current_group = any(current == f"docs/{slug}/" for slug, _ in entries)
            for slug, label in entries:
                if slug in NAV_TOPICS:
                    pages = NAV_TOPICS[slug]
                    topic_label = NAV_TOPIC_LABELS[locale][slug]
                    topic_aria = f"{topic_label}の関連ページ" if locale == "ja" else f"{topic_label} pages"
                    topic_open = " open" if any(current == f"docs/{page}/" for page in pages) else ""
                    topic_links = "".join(link(page, page_labels[page]) for page in pages)
                    links.append(f'<li><details class="docs-nav-topic" data-nav-topic="{slug}" aria-label="{html.escape(topic_aria, quote=True)}"{topic_open}><summary>{topic_label}</summary><ul>{topic_links}</ul></details></li>')
                elif slug not in topic_pages:
                    links.append(link(slug, label))
            opened = " open" if index == 0 or current_group else ""
            sections.append(f'<details class="docs-nav-group" data-nav-group="{entries[0][0]}"{opened}><summary>{title}</summary><ul>{"".join(links)}</ul></details>')
        return f'<nav class="docs-nav" aria-label="{LABELS[locale]["docs_contents"]}">' + "".join(sections) + "</nav>"

    def document(source, route, locale):
        if not source.is_file():
            raise ValueError(f"Missing Docs source: {source.relative_to(ROOT)}")
        md = MarkdownIt("commonmark", {"html": False}).enable("table").enable("strikethrough")
        labels = LABELS[locale]
        md.renderer.rules["fence"] = lambda tokens, index, options, env: code_block(tokens[index].content, tokens[index].info.split()[0] if tokens[index].info else "text", locale)
        md.renderer.rules["table_open"] = lambda *args: f'<div class="table-scroll" tabindex="0" role="region" aria-label="{labels["table_aria"]}"><table>\n'
        md.renderer.rules["table_close"] = lambda *args: '</table></div>\n'

        def table_cell_open(tokens, index, options, env):
            token = tokens[index]
            alignment = token.attrGet("style")
            for value in ("left", "center", "right"):
                if alignment == f"text-align:{value}":
                    token.attrs.pop("style")
                    token.attrJoin("class", f"align-{value}")
                    break
            return md.renderer.renderToken(tokens, index, options, env)

        md.renderer.rules["th_open"] = table_cell_open
        md.renderer.rules["td_open"] = table_cell_open

        def link_open(tokens, index, options, env):
            token = tokens[index]
            href = token.attrGet("href") or ""
            parsed = urlsplit(href)
            if parsed.path and not parsed.scheme and not parsed.netloc:
                target = (source.parent / unquote(parsed.path)).resolve()
                if not target.is_relative_to(ROOT) or not target.exists():
                    raise ValueError(f"Invalid source link in {source.relative_to(ROOT)}: {href}")
                if target in SOURCES or target in ENGLISH_SOURCES:
                    target_route = SOURCES.get(target, ENGLISH_SOURCES.get(target))
                    url = base + ("en/" if locale == "en" else "") + target_route
                elif target.is_relative_to(HERE / "assets"):
                    url = base + "assets/" + quote(target.relative_to(HERE / "assets").as_posix())
                else:
                    kind = "tree" if target.is_dir() else "blob"
                    url = f"{repo}/{kind}/{quote(ref, safe='')}/{quote(target.relative_to(ROOT).as_posix())}"
                if parsed.fragment:
                    url += "#" + parsed.fragment
                token.attrSet("href", url)
            return md.renderer.renderToken(tokens, index, options, env)

        md.renderer.rules["link_open"] = link_open
        render_image = md.renderer.rules["image"]

        def image(tokens, index, options, env):
            token = tokens[index]
            src = token.attrGet("src") or ""
            parsed = urlsplit(src)
            if parsed.path and not parsed.scheme and not parsed.netloc:
                target = (source.parent / unquote(parsed.path)).resolve()
                assets = HERE / "assets"
                if not target.is_relative_to(assets) or not target.is_file():
                    raise ValueError(f"Invalid Docs image in {source.relative_to(ROOT)}: {src}")
                token.attrSet("src", base + "assets/" + quote(target.relative_to(assets).as_posix()))
            return render_image(tokens, index, options, env)

        md.renderer.rules["image"] = image
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
        if headings:
            page_menu = f'<details class="mobile-page-nav"><summary>{labels["page_contents"]}</summary><nav aria-label="{labels["page_contents"]}"><ul>{outline}</ul></nav></details>'
            first_section = re.search(r'<h2\b', body)
            if first_section:
                body = body[:first_section.start()] + page_menu + body[first_section.start():]
        sidebar = navigation(route, locale)
        relative = quote(source.relative_to(ROOT).as_posix())
        meta = f'<div class="doc-meta"><span>{labels["docs_version"]}</span><a href="{repo}/blob/{quote(ref, safe="")}/{relative}">{labels["source_label"]}</a></div>'
        layout = f'<div class="docs-shell"><div class="docs-layout">{sidebar}<div class="docs-content"><details class="mobile-doc-nav"><summary>{labels["docs_contents"]}</summary>{sidebar}</details><main id="main" class="docs-main">{body}{meta}</main></div><nav class="outline" aria-label="{labels["page_contents"]}"><p>{labels["on_page"]}</p><ul>{outline}</ul></nav></div></div>'
        description = f"Nagi development documentation: {titles[0]}." if locale == "en" else f"開発中のNagiの日本語ドキュメント。{titles[0]}について説明します。"
        page(route, f"{titles[0]} | Nagi Docs", description, layout, locale, "docs-page")

    for locale, sources in (("ja", SOURCES), ("en", ENGLISH_SOURCES)):
        for source, route in sources.items():
            document(source, route, locale)

    example = (HERE / "examples/double.nagi").read_text(encoding="utf-8")
    for locale in ("ja", "en"):
        english = locale == "en"
        language_base = base + ("en/" if english else "")
        home_source = HERE / ("templates/home.en.html" if english else "templates/home.html")
        home = home_source.read_text(encoding="utf-8").replace("{{example}}", code_block(example, "nagi", locale))
        home = home.replace("{{base}}", language_base).replace("{{repo}}", html.escape(repo, quote=True))
        title = "Nagi — Readable code for backends." if english else "Nagi — 読みやすく書く、バックエンド。"
        description = "Nagi is a language in development for writing backends with Python-style syntax. It generates Rust code to build native executables and uses Rust libraries through adapters." if english else "Nagiは、Python風の構文でバックエンドを書くための開発中の言語です。Rustコードを生成して実行ファイルを作り、アダプターを通してRustのライブラリを使えます。"
        page("", title, description, home, locale)
        missing_title = "Page not found" if english else "ページが見つかりません"
        missing_body = "The link may have changed, or the URL may be incorrect." if english else "リンク先が変わったか、URLが間違っているようです。"
        contents_label = "Go to the Docs contents" if english else "Docsの目次へ"
        page("404.html", f"{missing_title} | Nagi", missing_title, f'<main id="main" class="landing section"><h1>{missing_title}</h1><p>{missing_body}</p><p><a href="{language_base}docs/">{contents_label}</a></p></main>', locale)
    (output / ".nojekyll").touch()
    locations = [origin + base + prefix + route for prefix in ("", "en/") for route in ["", *SOURCES.values()]]
    sitemap = '<?xml version="1.0" encoding="UTF-8"?><urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">' + "".join(f"<url><loc>{escape(url)}</loc></url>" for url in locations) + "</urlset>"
    (output / "sitemap.xml").write_text(sitemap, encoding="utf-8")
    pages = check_links(output, base)
    print(f"Built {pages} pages; all local links, anchors and assets verified: {output}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, default=ROOT / "build/website")
    parser.add_argument("--base-path")
    parser.add_argument("--site-url")
    parser.add_argument("--repository", default=DEFAULT_REPOSITORY)
    parser.add_argument("--ref", default="main")
    args = parser.parse_args()
    default_base, default_origin = site_defaults(args.repository)
    args.base_path = default_base if args.base_path is None else args.base_path
    args.site_url = default_origin if args.site_url is None else args.site_url
    base = "/" + args.base_path.strip("/") + "/" if args.base_path.strip("/") else "/"
    if re.search(r"[^A-Za-z0-9_./-]", base) or any(p in {".", ".."} for p in base.split("/")):
        parser.error("base-path must be a URL path without . or .. segments")
    if urlsplit(args.site_url).scheme != "https" or urlsplit(args.repository).scheme != "https":
        parser.error("site-url and repository must use HTTPS")
    build(args.out, base, args.site_url.rstrip("/"), args.repository.rstrip("/"), args.ref)


if __name__ == "__main__":
    main()
