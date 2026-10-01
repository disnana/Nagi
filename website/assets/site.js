'use strict';
const english = document.documentElement.lang === 'en';
const copyLabel = english ? 'Copy' : 'コピー';
const copiedLabel = english ? 'Copied' : 'コピーしました';
for (const button of document.querySelectorAll('[data-copy]')) {
  button.addEventListener('click', async () => {
    const code = button.closest('.code-block')?.querySelector('code');
    if (!code) return;
    try {
      await navigator.clipboard.writeText(code.textContent);
      document.getElementById('copy-status').textContent = english ? 'Code copied.' : 'コードをコピーしました。';
      button.textContent = copiedLabel;
      setTimeout(() => { button.textContent = copyLabel; }, 2000);
    } catch {
      document.getElementById('copy-status').textContent = english ? 'Could not copy. Select the code and copy it manually.' : 'コピーできませんでした。コードを選択してコピーしてください。';
    }
  });
}
