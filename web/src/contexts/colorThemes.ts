import themesData from './themes.json';

// Volt design system: a single, brand-aligned theme set (Dark / Light /
// Paper) shared with the Volt desktop client. The old 30-plus editor themes
// and per-accent switcher were removed in favour of this fixed brand palette.
export type ColorThemeId = 'volt-dark' | 'volt-light' | 'volt-paper';

export interface ColorThemeDef {
  id: ColorThemeId;
  name: string;
  scheme: 'dark' | 'light';
  preview: [string, string, string, string, string];
  vars: Record<string, string>;
}

export const colorThemes: ColorThemeDef[] = themesData as unknown as ColorThemeDef[];

export const colorThemeMap: Record<ColorThemeId, ColorThemeDef> =
  Object.fromEntries(colorThemes.map(t => [t.id, t])) as Record<ColorThemeId, ColorThemeDef>;

export const DEFAULT_DARK_THEME: ColorThemeId = 'volt-dark';
export const DEFAULT_LIGHT_THEME: ColorThemeId = 'volt-light';
