import { useSyncExternalStore } from 'react';

type Theme = 'light' | 'dark';

const STORAGE_KEY = 'theme';

// Module-level state — no browser API accessed at module load time (SSR-safe)
let _theme: Theme | null = null;
let _mediaQuery: MediaQueryList | null = null;
const _listeners = new Set<() => void>();

function getInitialTheme(): Theme {
  const stored = localStorage.getItem(STORAGE_KEY);
  if (stored === 'light' || stored === 'dark') return stored;
  return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
}

function applyTheme(t: Theme) {
  document.documentElement.classList.toggle('dark', t === 'dark');
  localStorage.setItem(STORAGE_KEY, t);
}

function ensureInitialized(): Theme {
  if (_theme === null) {
    _theme = getInitialTheme();
    applyTheme(_theme);

    // Listen for system theme changes (only if user hasn't set a preference)
    _mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
    _mediaQuery.addEventListener('change', handleSystemThemeChange);
  }
  return _theme;
}

function handleSystemThemeChange(e: MediaQueryListEvent) {
  // Only follow system theme if user hasn't explicitly set a preference
  const stored = localStorage.getItem(STORAGE_KEY);
  if (stored === 'light' || stored === 'dark') return;
  _theme = e.matches ? 'dark' : 'light';
  applyTheme(_theme);
  notify();
}

function notify() {
  _listeners.forEach((l) => l());
}

export function subscribe(listener: () => void) {
  ensureInitialized();
  _listeners.add(listener);
  return () => {
    _listeners.delete(listener);
  };
}

export function getTheme(): Theme {
  return ensureInitialized();
}

export function toggleTheme() {
  const current = ensureInitialized();
  _theme = current === 'dark' ? 'light' : 'dark';
  applyTheme(_theme);
  notify();
}

export function isDark(): boolean {
  return ensureInitialized() === 'dark';
}

// React hook — uses useSyncExternalStore to prevent tearing in Concurrent mode
export function useTheme() {
  const theme = useSyncExternalStore(subscribe, getTheme, getServerSnapshot);
  return { theme, isDark: theme === 'dark', toggleTheme };
}

// Server snapshot — default to 'light' since we can't access browser API on server
function getServerSnapshot(): Theme {
  return 'light';
}
