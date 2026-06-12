import { createSignal } from 'solid-js'

type Theme = 'light' | 'dark'

const STORAGE_KEY = 'virs-theme'

function getInitialTheme(): Theme {
  const stored = localStorage.getItem(STORAGE_KEY)
  if (stored === 'light' || stored === 'dark') return stored
  return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'
}

const [theme, setTheme] = createSignal<Theme>(getInitialTheme())

function applyTheme(t: Theme) {
  document.documentElement.classList.toggle('dark', t === 'dark')
  localStorage.setItem(STORAGE_KEY, t)
}

// Apply on load
applyTheme(theme())

export function getTheme() {
  return theme()
}

export function toggleTheme() {
  const next = theme() === 'dark' ? 'light' : 'dark'
  setTheme(next)
  applyTheme(next)
}

export function isDark() {
  return theme() === 'dark'
}
