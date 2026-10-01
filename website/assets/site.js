'use strict';
for (const button of document.querySelectorAll('[data-copy]')) {
  button.addEventListener('click', async () => {
    const code = button.closest('.code-block')?.querySelector('code');
    if (!code) return;
    try {
      await navigator.clipboard.writeText(code.textContent);
      document.getElementById('copy-status').textContent = 'コードをコピーしました。';
      button.textContent = 'コピーしました';
      setTimeout(() => { button.textContent = 'コピー'; }, 2000);
    } catch {
      document.getElementById('copy-status').textContent = 'コピーできませんでした。コードを選択してコピーしてください。';
    }
  });
}
