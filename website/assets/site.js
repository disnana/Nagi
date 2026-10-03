'use strict';
const navs = [...document.querySelectorAll('.docs-nav')];
if (navs.length) {
  const navigationKey = `nagi:docs-nav:${navs[0].querySelector('.docs-nav-title').getAttribute('href')}`;
  const groupsKey = `${navigationKey}:groups`;
  let groups = {};
  try {
    const saved = JSON.parse(sessionStorage.getItem(groupsKey));
    if (saved && typeof saved === 'object' && !Array.isArray(saved)) groups = saved;
  } catch { /* Native disclosure buttons also work without browser storage. */ }
  const menus = navs.flatMap(nav => [...nav.querySelectorAll('[data-nav-group]')]);
  for (const menu of menus) {
    const name = menu.dataset.navGroup;
    if (menu.querySelector('[aria-current="page"]')) {
      menu.open = true;
    } else if (typeof groups[name] === 'boolean') {
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
  const topics = navs.flatMap(nav => [...nav.querySelectorAll('[data-nav-topic]')]);
  for (const topic of topics) {
    topic.open = Boolean(topic.querySelector('[aria-current="page"]'));
    topic.addEventListener('toggle', () => {
      for (const other of topics) {
        if (other !== topic && other.dataset.navTopic === topic.dataset.navTopic && other.open !== topic.open) other.open = topic.open;
      }
    });
  }
  const mobileMenu = document.querySelector('.mobile-doc-nav');
  if (mobileMenu) {
    const mobileKey = `${navigationKey}:mobile`;
    try {
      const saved = sessionStorage.getItem(mobileKey);
      if (saved === 'true' || saved === 'false') mobileMenu.open = saved === 'true';
    } catch { /* The menu remains usable without browser storage. */ }
    mobileMenu.querySelector('summary').addEventListener('click', () => {
      try { sessionStorage.setItem(mobileKey, String(!mobileMenu.open)); } catch { /* Keep links usable. */ }
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
      const current = sidebar.querySelector('[aria-current="page"]');
      if (!current || current.closest('details:not([open])') || !current.getClientRects().length) return;
      const item = current.getBoundingClientRect();
      const bounds = sidebar.getBoundingClientRect();
      if (item.top < bounds.top) {
        sidebar.scrollTop += item.top - bounds.top;
        position = sidebar.scrollTop;
      } else if (item.bottom > bounds.bottom) {
        sidebar.scrollTop += item.bottom - bounds.bottom;
        position = sidebar.scrollTop;
      }
    } else {
      const current = sidebar.querySelector('[aria-current="page"]');
      if (!current || current.closest('details:not([open])') || !current.getClientRects().length) return;
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
