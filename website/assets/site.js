'use strict';
const navs = [...document.querySelectorAll('.docs-nav')];
if (navs.length) {
  const groupsKey = `nagi:docs-nav:${navs[0].querySelector('.docs-nav-title').getAttribute('href')}:groups`;
  let groups = {};
  try {
    const saved = JSON.parse(sessionStorage.getItem(groupsKey));
    if (saved && typeof saved === 'object' && !Array.isArray(saved)) groups = saved;
  } catch { /* Native disclosure buttons also work without browser storage. */ }
  const menus = navs.flatMap(nav => [...nav.querySelectorAll('[data-nav-group]')]);
  for (const menu of menus) {
    const name = menu.dataset.navGroup;
    if (typeof groups[name] === 'boolean') {
      menu.open = groups[name];
    }
    menu.querySelector('summary').addEventListener('click', () => {
      // Native summary buttons toggle after their click handlers run.
      groups[name] = !menu.open;
      try { sessionStorage.setItem(groupsKey, JSON.stringify(groups)); } catch { /* Keep links usable. */ }
    });
    menu.addEventListener('toggle', () => {
      for (const other of menus) {
        if (other !== menu && other.dataset.navGroup === name && other.open !== menu.open) other.open = menu.open;
      }
    });
  }
}

const sidebar = document.querySelector('.docs-layout > .docs-nav');
if (sidebar) {
  const key = `nagi:docs-nav:${sidebar.querySelector('.docs-nav-title').getAttribute('href')}`;
  let position = null;
  try {
    const saved = sessionStorage.getItem(key);
    if (saved !== null && Number.isFinite(Number(saved)) && Number(saved) >= 0) position = Number(saved);
  } catch { /* Navigation also works when browser storage is unavailable. */ }

  const restore = () => {
    if (!sidebar.clientHeight) return;
    if (position !== null) {
      sidebar.scrollTop = position;
    } else {
      const current = sidebar.querySelector('[aria-current="page"]');
      if (!current) return;
      const item = current.getBoundingClientRect();
      const bounds = sidebar.getBoundingClientRect();
      if (item.top < bounds.top || item.bottom > bounds.bottom) {
        sidebar.scrollTop += item.top - bounds.top - (sidebar.clientHeight - item.height) / 2;
      }
    }
  };
  const remember = () => {
    if (!sidebar.clientHeight) return;
    position = sidebar.scrollTop;
    try { sessionStorage.setItem(key, String(position)); } catch { /* Keep links usable. */ }
  };
  restore();
  sidebar.addEventListener('scroll', remember, { passive: true });
  sidebar.addEventListener('click', remember);
  window.addEventListener('pagehide', remember);
  window.addEventListener('pageshow', restore);
  window.addEventListener('resize', restore);
}

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
