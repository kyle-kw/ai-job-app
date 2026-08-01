import type { AppTheme } from '$lib/types';

export type ResolvedAppTheme = Exclude<AppTheme, 'system'> | 'dark';

export interface AppThemeOption {
  id: AppTheme;
  label: string;
  description: string;
  swatches: [string, string, string];
}

export const APP_THEMES: AppThemeOption[] = [
  {
    id: 'system',
    label: '跟随系统',
    description: '自动切换浅色与深色',
    swatches: ['#F5F6F2', '#176B57', '#151A18']
  },
  {
    id: 'forest',
    label: '森林绿',
    description: '自然、专注的默认浅色',
    swatches: ['#F5F6F2', '#E4F1EC', '#176B57']
  },
  {
    id: 'mist',
    label: '雾蓝',
    description: '清爽、克制的蓝灰浅色',
    swatches: ['#F4F7FA', '#E2EFF6', '#2F6F91']
  },
  {
    id: 'lavender',
    label: '淡紫',
    description: '柔和、安静的紫调浅色',
    swatches: ['#F8F6FA', '#EEE7F4', '#72518F']
  },
  {
    id: 'apricot',
    label: '暖杏',
    description: '温暖、轻盈的杏色浅色',
    swatches: ['#FAF7F2', '#F6E6DA', '#99502B']
  }
];

export function resolveAppTheme(theme: AppTheme, prefersDark: boolean): ResolvedAppTheme {
  if (theme === 'system') return prefersDark ? 'dark' : 'forest';
  return theme;
}

export function systemPrefersDark(): boolean {
  return (
    typeof window !== 'undefined' &&
    typeof window.matchMedia === 'function' &&
    window.matchMedia('(prefers-color-scheme: dark)').matches
  );
}

export function applyAppTheme(
  theme: AppTheme,
  prefersDark: boolean,
  root: HTMLElement = document.documentElement
): ResolvedAppTheme {
  const resolved = resolveAppTheme(theme, prefersDark);
  root.dataset.theme = resolved;
  return resolved;
}
