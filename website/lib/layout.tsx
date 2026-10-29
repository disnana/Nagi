import type { BaseLayoutProps } from 'fumadocs-ui/layouts/shared';
import { i18n } from '@/lib/i18n';
import { uiTranslations } from 'fumadocs-ui/i18n';
import { LocaleSwitch } from '@/components/locale-switch';

export const translations = i18n
  .translations()
  .extend(uiTranslations())
  .add({
    ja: {
      displayName: '日本語',
      'Ask AI(AI chat button)': 'AIに質問',
      'Back to Home(404 page)': 'ホームへ戻る',
      'Choose a language(language switcher)': '言語を選択',
      'Choose a language(language switcher)(aria-label)': '表示言語を選択',
      'Close Banner(banner)(aria-label)': 'お知らせを閉じる',
      'Close Search(search dialog)(aria-label)': '検索を閉じる',
      'Close Sidebar(aria-label)': 'サイドバーを閉じる',
      'Close Sidebar(sidebar)(aria-label)': 'サイドバーを閉じる',
      'Close(image zoom)(aria-label)': '画像を閉じる',
      'Collapse Sidebar(sidebar)(aria-label)': 'サイドバーを折りたたむ',
      'Copied Anchor Link(heading anchor)(aria-label)': '見出しリンクをコピーしました',
      'Copied Link(accordion)(aria-label)': 'リンクをコピーしました',
      'Copied Markdown(page actions)': 'Markdownをコピーしました',
      'Copied Text(code block)(aria-label)': 'コードをコピーしました',
      'Copy Anchor Link(heading anchor)(aria-label)': '見出しリンクをコピー',
      'Copy Link(accordion)(aria-label)': 'リンクをコピー',
      'Copy Markdown(page actions)': 'Markdownをコピー',
      'Copy Text(code block)(aria-label)': 'コードをコピー',
      'Dark(theme switcher)(aria-label)': 'ダーク',
      'Default(type table)': '既定値',
      'Edit on GitHub(edit page)': 'GitHubで編集',
      'Hide Sidebar(sidebar)': 'サイドバーを隠す',
      'Image Viewer(image zoom)(aria-label)': '画像ビューアー',
      'Last updated on(page footer)': '最終更新',
      'Layout Tab(layout tab trigger)': 'レイアウト',
      'Light(theme switcher)(aria-label)': 'ライト',
      'Next Image(image zoom)(aria-label)': '次の画像',
      'Next Page(pagination)': '次のページ',
      'No Headings(table of contents)': '見出しはありません',
      'No results found(search dialog)': '検索結果がありません',
      'On this page(table of contents)': 'このページの内容',
      'Open Search(search trigger)(aria-label)': '検索を開く',
      'Open Sidebar(sidebar)(aria-label)': 'サイドバーを開く',
      'Open in ChatGPT(page actions)': 'ChatGPTで開く',
      'Open in Claude(page actions)': 'Claudeで開く',
      'Open in Cursor(page actions)': 'Cursorで開く',
      'Open in GitHub(page actions)': 'GitHubで開く',
      'Open in Scira AI(page actions)': 'Scira AIで開く',
      'Open(page actions)': '開く',
      'Page Not Found(404 page)': 'ページが見つかりません',
      'Parameters(type table)': '引数',
      'Previous Image(image zoom)(aria-label)': '前の画像',
      'Previous Page(pagination)': '前のページ',
      'Prop(type table)': 'プロパティ',
      'Read {url}, I want to ask questions about it.(page actions)': '{url}を読んで、内容について質問します。',
      'Returns(type table)': '戻り値',
      'Search failed(search dialog)': '検索に失敗しました',
      'Search failed, showing results for {query}(search dialog)': '検索に失敗したため、{query}の結果を表示します',
      'Search(search dialog)': 'ドキュメントを検索',
      'Search(search trigger)': '検索',
      'Show Sidebar(sidebar)': 'サイドバーを表示',
      'System(theme switcher)(aria-label)': 'システム設定',
      'Table of Contents(inline table of contents)': '目次',
      'The page you are looking for might have been removed, had its name changed, or is temporarily unavailable.(404 page)':
        'お探しのページは削除、移動されたか、一時的に利用できません。',
      'Toggle Menu(mobile menu)(aria-label)': 'メニューを切り替える',
      'Toggle Theme(theme switcher)(aria-label)': '外観を切り替える',
      'Type(type table)': '型',
      'View as Markdown(page actions)': 'Markdownで表示',
    },
    en: { displayName: 'English' },
  });

export function baseOptions(locale: 'ja' | 'en'): BaseLayoutProps {
  const docsUrl = locale === 'ja' ? '/docs' : '/en/docs';

  return {
    i18n: false,
    nav: {
      title: 'Nagi',
      url: locale === 'ja' ? '/' : '/en',
    },
    githubUrl: 'https://github.com/disnana/Nagi',
    links: [
      {
        type: 'main',
        text: locale === 'ja' ? '書き方を学ぶ' : 'Learn Nagi',
        url: docsUrl,
      },
      {
        type: 'main',
        text: locale === 'ja' ? '変更履歴' : 'Changelog',
        url: `${docsUrl}/changelog`,
      },
      {
        type: 'custom',
        secondary: true,
        children: <LocaleSwitch locale={locale} />,
      },
    ],
  };
}
