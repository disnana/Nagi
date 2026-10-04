'use strict';
(() => {
  const storageKey = 'nagi:theme';
  const system = window.matchMedia('(prefers-color-scheme: dark)');
  const valid = value => value === 'light' || value === 'dark' ? value : 'system';
  let preference = 'system';
  try { preference = valid(localStorage.getItem(storageKey)); } catch { /* Follow the system when storage is unavailable. */ }

  const apply = () => {
    document.documentElement.dataset.theme = preference === 'system' ? (system.matches ? 'dark' : 'light') : preference;
    const select = document.getElementById('theme-select');
    if (select) select.value = preference;
  };
  // Run before stylesheets load so a saved preference also applies on navigation.
  apply();
  system.addEventListener('change', apply);
  window.addEventListener('storage', event => {
    if (event.key !== storageKey && event.key !== null) return;
    preference = valid(event.newValue);
    apply();
  });
  document.addEventListener('DOMContentLoaded', () => {
    const select = document.getElementById('theme-select');
    if (!select) return;
    apply();
    select.closest('.theme-control').hidden = false;
    select.addEventListener('change', () => {
      preference = valid(select.value);
      apply();
      try {
        if (preference === 'system') localStorage.removeItem(storageKey);
        else localStorage.setItem(storageKey, preference);
      } catch { /* Changing the appearance still works without browser storage. */ }
    });
  });
})();
