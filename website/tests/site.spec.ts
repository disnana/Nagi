import AxeBuilder from '@axe-core/playwright';
import { expect, test } from '@playwright/test';

const basePath = process.env.PAGES_BASE_PATH ?? '';
const sitePath = (path: string) => `${basePath}${path}`;

test('Japanese and English pages remain separate and navigable', async ({ page }) => {
  await page.goto(sitePath('/docs/writing/'));
  await expect(page.locator('html')).toHaveAttribute('lang', 'ja');
  await expect(page.getByRole('heading', { level: 1 })).toContainText('Nagi');
  await expect(page.getByRole('button', { name: '検索を開く' })).toBeVisible();

  await page.goto(sitePath('/en/docs/writing/'));
  await expect(page.locator('html')).toHaveAttribute('lang', 'en');
  await expect(page.getByRole('heading', { level: 1 })).toContainText('Nagi');
  await expect(page.getByRole('button', { name: 'Open Search' })).toBeVisible();
});

test('language switch preserves the equivalent docs route', async ({ page }) => {
  await page.goto(sitePath('/docs/writing/'));
  await page.getByRole('link', { name: 'English', exact: true }).click();
  await expect(page).toHaveURL(/\/en\/docs\/writing\/$/);
  await expect(page.locator('html')).toHaveAttribute('lang', 'en');
  await page.getByRole('link', { name: '日本語', exact: true }).click();
  await expect(page).toHaveURL(/\/docs\/writing\/$/);
  await expect(page.locator('html')).toHaveAttribute('lang', 'ja');
});

test('static search is lazy, keyboard accessible, and locale scoped', async ({ page }) => {
  const searchRequests: string[] = [];
  page.on('request', (request) => {
    if (request.url().includes('/api/search/')) searchRequests.push(request.url());
  });
  await page.goto(sitePath('/docs/writing/'));
  expect(searchRequests).toEqual([]);
  await page.keyboard.press(process.platform === 'darwin' ? 'Meta+KeyK' : 'Control+KeyK');
  const input = page.getByRole('combobox', { name: 'ドキュメントを検索' });
  await expect(input).toBeFocused();
  // Opening the dialog must not eagerly download the index. It is fetched only
  // after the reader starts a search, so it stays off the initial page path.
  expect(searchRequests).toEqual([]);
  await input.fill('書き始める');
  await expect.poll(() => searchRequests.length).toBeGreaterThan(0);
  expect(searchRequests.every((url) => url.endsWith('/api/search/ja'))).toBe(true);
  const result = page.getByRole('option', { name: /Nagiを書き始める/ });
  await expect(result).toBeVisible();
  await result.click();
  await expect(page).toHaveURL(/\/docs\/writing\/$/);
});

for (const width of [360, 390]) {
  test(`code blocks copy and stay inside a ${width}px viewport`, async ({ page }) => {
    await page.setViewportSize({ width, height: 800 });
    await page.goto(sitePath('/docs/writing/'));
    const copy = page.getByRole('button', { name: 'コードをコピー' }).first();
    await expect(copy).toBeVisible();
    await copy.click();
    await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toContain('def main() -> Result[unit, Error]:');
    const overflow = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
    expect(overflow).toBeLessThanOrEqual(1);
  });
}

test('theme control applies dark mode without accessibility regressions', async ({ page }) => {
  await page.goto(sitePath('/docs/writing/'));
  await page.getByRole('button', { name: '外観を切り替える' }).click();
  await expect(page.locator('html')).toHaveClass(/dark/);
  const result = await new AxeBuilder({ page }).analyze();
  expect(result.violations.filter((item) => item.impact === 'serious' || item.impact === 'critical')).toEqual([]);
});

for (const route of ['/docs/writing/', '/en/docs/writing/']) {
  test(`${route} has no serious accessibility violations`, async ({ page }) => {
    await page.goto(sitePath(route));
    const result = await new AxeBuilder({ page }).analyze();
    expect(result.violations.filter((item) => item.impact === 'serious' || item.impact === 'critical')).toEqual([]);
  });
}

test('legacy aliases are physical pages with canonical locale routes', async ({ page }) => {
  await page.goto(sitePath('/docs/use-cases/'));
  await expect(page.getByRole('heading', { level: 1 })).toHaveText('このページは移動しました');
  await expect(page.locator('link[rel="canonical"]')).toHaveAttribute(
    'href',
    'https://nagi.disnana.com/docs/writing/',
  );
  await page.getByRole('link', { name: '新しいページを開く' }).click();
  await expect(page).toHaveURL(/\/docs\/writing\/$/);
});

test('versioned changelog permalink keeps locale alternates', async ({ page }) => {
  await page.goto(sitePath('/docs/changelog/nagi-0-2-2/'));
  await expect(page.getByRole('heading', { name: 'Nagi 0.2.2', exact: true })).toBeVisible();
  await expect(page.locator('link[hreflang="en"]')).toHaveAttribute(
    'href',
    'https://nagi.disnana.com/en/docs/changelog/nagi-0-2-2/',
  );
});
