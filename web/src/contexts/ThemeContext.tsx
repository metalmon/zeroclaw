import { createContext, useState, useEffect, useCallback, type ReactNode } from 'react';
import { colorThemeMap, DEFAULT_DARK_THEME, DEFAULT_LIGHT_THEME, type ColorThemeId } from './colorThemes';

// ── Types ─────────────────────────────────────────────────────────────────────

// Volt theme set, mirroring the desktop client: System follows the OS, Dark /
// Light are the brand neutrals, Paper is the fixed warm "sand" theme. The old
// per-accent switcher and OLED mode were dropped — the brand accent is fixed.
export type ThemeMode = 'system' | 'dark' | 'light' | 'paper';
export type UiFont = 'system' | 'inter' | 'segoe' | 'sf';
export type MonoFont = 'jetbrains' | 'fira' | 'cascadia' | 'system-mono';

export const uiFontStacks: Record<UiFont, string> = {
  system: 'system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif',
  inter: '"Inter", system-ui, sans-serif',
  segoe: '"Segoe UI", system-ui, sans-serif',
  sf: '-apple-system, BlinkMacSystemFont, "SF Pro Text", sans-serif',
};

export const monoFontStacks: Record<MonoFont, string> = {
  jetbrains: '"JetBrains Mono", "Fira Code", "Cascadia Code", monospace',
  fira: '"Fira Code", "JetBrains Mono", "Cascadia Code", monospace',
  cascadia: '"Cascadia Code", "JetBrains Mono", "Fira Code", monospace',
  'system-mono': 'ui-monospace, "SF Mono", "Cascadia Code", "Fira Code", monospace',
};

export interface ThemeContextValue {
  theme: ThemeMode;
  uiFont: UiFont;
  monoFont: MonoFont;
  uiFontSize: number;
  monoFontSize: number;
  resolvedTheme: 'dark' | 'light';
  setTheme: (t: ThemeMode) => void;
  setUiFont: (f: UiFont) => void;
  setMonoFont: (f: MonoFont) => void;
  setUiFontSize: (size: number) => void;
  setMonoFontSize: (size: number) => void;
}

export const ThemeContext = createContext<ThemeContextValue>({
  theme: 'dark',
  uiFont: 'system',
  monoFont: 'jetbrains',
  uiFontSize: 15,
  monoFontSize: 14,
  resolvedTheme: 'dark',
  setTheme: () => {},
  setUiFont: () => {},
  setMonoFont: () => {},
  setUiFontSize: () => {},
  setMonoFontSize: () => {},
});

// ── Font loader ───────────────────────────────────────────────────────────────

const loadedFonts: Set<string> = new Set();

function loadGoogleFont(family: string, weights: string = '400;500;600') {
  const id = `gfont-${family.replace(/\s+/g, '-').toLowerCase()}`;
  if (loadedFonts.has(id)) return;
  loadedFonts.add(id);
  const link = document.createElement('link');
  link.id = id;
  link.rel = 'stylesheet';
  link.href = `https://fonts.googleapis.com/css2?family=${encodeURIComponent(family)}:wght@${weights}&display=swap`;
  document.head.appendChild(link);
}

function loadUiFont(font: string) {
  if (font === 'inter') loadGoogleFont('Inter');
  if (font === 'segoe') loadGoogleFont('Segoe UI');
  if (font === 'sf') loadGoogleFont('SF Pro Text');
}

function loadMonoFont(font: string) {
  if (font === 'jetbrains') loadGoogleFont('JetBrains Mono');
  if (font === 'fira') loadGoogleFont('Fira Code');
  if (font === 'cascadia') loadGoogleFont('Cascadia Code');
}

// ── Locale storage ────────────────────────────────────────────────────────────

export const LOCALE_STORAGE_KEY = 'zeroclaw-locale';

export function loadLocale(): string {
  return localStorage.getItem(LOCALE_STORAGE_KEY) ?? 'en';
}

export function saveLocale(locale: string) {
  localStorage.setItem(LOCALE_STORAGE_KEY, locale);
}

/**
 * Whether the user has ever explicitly picked a locale (persisted via
 * `saveLocale`). Distinguishes "no stored key yet" from "stored key happens
 * to be 'en'" — the former should still defer to the server/enterprise
 * default and the browser language, the latter is a real user choice that
 * must win over both.
 */
export function hasExplicitLocale(): boolean {
  return localStorage.getItem(LOCALE_STORAGE_KEY) !== null;
}

// ── Theme storage ─────────────────────────────────────────────────────────────

const STORAGE_KEY = 'zeroclaw-theme';

interface StoredTheme {
  theme: ThemeMode;
  uiFont: UiFont;
  monoFont: MonoFont;
  uiFontSize: number;
  monoFontSize: number;
}

const DEFAULTS: StoredTheme = {
  theme: 'system',
  uiFont: 'system',
  monoFont: 'jetbrains',
  uiFontSize: 15,
  monoFontSize: 14,
};

const validThemes: ThemeMode[] = ['system', 'dark', 'light', 'paper'];

function loadStored(): StoredTheme {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (raw) {
      const parsed = JSON.parse(raw);
      // Legacy stores (accent/colorTheme/oled) collapse to the closest Volt
      // mode: any non-Volt value falls through to the System default.
      const theme: ThemeMode = validThemes.includes(parsed.theme) ? parsed.theme : DEFAULTS.theme;
      const uiFont: UiFont = uiFontStacks[parsed.uiFont as UiFont] ? parsed.uiFont as UiFont : DEFAULTS.uiFont;
      const monoFont: MonoFont = monoFontStacks[parsed.monoFont as MonoFont] ? parsed.monoFont as MonoFont : DEFAULTS.monoFont;
      const uiFontSize = Number.isFinite(parsed.uiFontSize) ? Math.min(20, Math.max(12, Number(parsed.uiFontSize))) : DEFAULTS.uiFontSize;
      const monoFontSize = Number.isFinite(parsed.monoFontSize) ? Math.min(20, Math.max(12, Number(parsed.monoFontSize))) : DEFAULTS.monoFontSize;
      return { theme, uiFont, monoFont, uiFontSize, monoFontSize };
    }
  } catch { /* ignore corrupt storage */ }
  return DEFAULTS;
}

// ── Provider ───────────────────────────────────────────────────────────────────

function applyVars(vars: Record<string, string>) {
  const root = document.documentElement;
  for (const [k, v] of Object.entries(vars)) {
    if (k === '--color-scheme') {
      root.style.colorScheme = v as 'light' | 'dark';
    } else {
      root.style.setProperty(k, v);
    }
  }
}

/** Resolve a theme mode to a concrete Volt color-theme id. */
function resolveColorTheme(mode: ThemeMode): ColorThemeId {
  switch (mode) {
    case 'dark': return 'volt-dark';
    case 'light': return 'volt-light';
    case 'paper': return 'volt-paper';
    case 'system':
    default: {
      const preferLight = window.matchMedia('(prefers-color-scheme: light)').matches;
      return preferLight ? DEFAULT_LIGHT_THEME : DEFAULT_DARK_THEME;
    }
  }
}

function resolveThemeScheme(mode: ThemeMode): 'dark' | 'light' {
  const ct = colorThemeMap[resolveColorTheme(mode)];
  return ct?.scheme ?? 'dark';
}

function fontVars(uiFont: UiFont, monoFont: MonoFont, uiFontSize: number, monoFontSize: number) {
  return {
    '--pc-font-ui': uiFontStacks[uiFont],
    '--pc-font-mono': monoFontStacks[monoFont],
    '--pc-font-size': `${uiFontSize}px`,
    '--pc-font-size-mono': `${monoFontSize}px`,
  };
}

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [stored] = useState(loadStored);
  const [theme, setThemeState] = useState<ThemeMode>(stored.theme);
  const [uiFont, setUiFontState] = useState<UiFont>(stored.uiFont);
  const [monoFont, setMonoFontState] = useState<MonoFont>(stored.monoFont);
  const [uiFontSize, setUiFontSizeState] = useState<number>(stored.uiFontSize);
  const [monoFontSize, setMonoFontSizeState] = useState<number>(stored.monoFontSize);

  const persist = useCallback((s: StoredTheme) => {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(s));
  }, []);

  const applyAll = useCallback((s: StoredTheme) => {
    const ct = colorThemeMap[resolveColorTheme(s.theme)];
    const themeVars = ct?.vars ?? colorThemeMap[DEFAULT_DARK_THEME].vars;
    applyVars({
      ...themeVars,
      ...fontVars(s.uiFont, s.monoFont, s.uiFontSize, s.monoFontSize),
    });
  }, []);

  const setTheme = useCallback((t: ThemeMode) => {
    setThemeState(t);
    const next = { theme: t, uiFont, monoFont, uiFontSize, monoFontSize };
    applyAll(next);
    persist(next);
  }, [uiFont, monoFont, uiFontSize, monoFontSize, applyAll, persist]);

  const setUiFont = useCallback((f: UiFont) => {
    setUiFontState(f);
    loadUiFont(f);
    const next: StoredTheme = { theme, uiFont: f, monoFont, uiFontSize, monoFontSize };
    applyAll(next);
    persist(next);
  }, [theme, applyAll, persist, monoFont, uiFontSize, monoFontSize]);

  const setMonoFont = useCallback((f: MonoFont) => {
    setMonoFontState(f);
    loadMonoFont(f);
    const next: StoredTheme = { theme, uiFont, monoFont: f, uiFontSize, monoFontSize };
    applyAll(next);
    persist(next);
  }, [theme, applyAll, persist, uiFont, uiFontSize, monoFontSize]);

  const setUiFontSize = useCallback((size: number) => {
    const clamped = Math.min(20, Math.max(12, size));
    setUiFontSizeState(clamped);
    const next: StoredTheme = { theme, uiFont, monoFont, uiFontSize: clamped, monoFontSize };
    applyAll(next);
    persist(next);
  }, [theme, applyAll, persist, uiFont, monoFont, monoFontSize]);

  const setMonoFontSize = useCallback((size: number) => {
    const clamped = Math.min(20, Math.max(12, size));
    setMonoFontSizeState(clamped);
    const next: StoredTheme = { theme, uiFont, monoFont, uiFontSize, monoFontSize: clamped };
    applyAll(next);
    persist(next);
  }, [theme, applyAll, persist, uiFont, monoFont, uiFontSize]);

  useEffect(() => {
    applyAll({ theme, uiFont, monoFont, uiFontSize, monoFontSize });
    loadUiFont(uiFont);
    loadMonoFont(monoFont);
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    if (theme !== 'system') return;
    const mq = window.matchMedia('(prefers-color-scheme: light)');
    const handler = () => applyAll({ theme, uiFont, monoFont, uiFontSize, monoFontSize });
    mq.addEventListener('change', handler);
    return () => mq.removeEventListener('change', handler);
  }, [theme, applyAll, uiFont, monoFont, uiFontSize, monoFontSize]);

  const resolvedTheme = resolveThemeScheme(theme);

  const value: ThemeContextValue = {
    theme, uiFont, monoFont, uiFontSize, monoFontSize,
    resolvedTheme, setTheme, setUiFont, setMonoFont, setUiFontSize, setMonoFontSize,
  };

  return <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>;
}
