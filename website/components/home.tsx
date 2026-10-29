import Link from 'next/link';

export function Home({ locale }: { locale: 'ja' | 'en' }) {
  const ja = locale === 'ja';
  const docs = ja ? '/docs' : '/en/docs';
  return (
    <main className="mx-auto flex w-full max-w-5xl flex-1 flex-col justify-center gap-8 px-6 py-20">
      <p className="text-sm font-medium text-fd-muted-foreground">Nagi 0.2.2</p>
      <h1 className="max-w-3xl text-5xl font-bold tracking-tight">
        {ja ? 'Nagiの書き方を、動くコードから学ぶ' : 'Learn Nagi from working code'}
      </h1>
      <p className="max-w-2xl text-lg text-fd-muted-foreground">
        {ja
          ? '最小のプログラムからHTTP、SQLite、非同期処理、Actorまで、実行方法と結果を一つずつ確認できます。'
          : 'Start with the smallest program, then build HTTP, SQLite, async, and Actor applications with exact commands and results.'}
      </p>
      <div className="flex flex-wrap gap-3">
        <Link className="rounded-lg bg-fd-primary px-5 py-3 font-medium text-fd-primary-foreground" href={docs}>
          {ja ? '書き始める' : 'Start writing'}
        </Link>
        <Link className="rounded-lg border px-5 py-3 font-medium" href={ja ? '/en' : '/'} hrefLang={ja ? 'en' : 'ja'}>
          {ja ? 'English' : '日本語'}
        </Link>
      </div>
    </main>
  );
}
