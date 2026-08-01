import { describe, expect, it } from 'vitest';
import { APP_THEMES, applyAppTheme, resolveAppTheme } from './theme';

describe('application theme', () => {
  it('exposes the supported theme preferences in settings order', () => {
    expect(APP_THEMES.map((theme) => theme.id)).toEqual([
      'system',
      'forest',
      'mist',
      'lavender',
      'apricot'
    ]);
  });

  it('resolves the system preference to forest or dark', () => {
    expect(resolveAppTheme('system', false)).toBe('forest');
    expect(resolveAppTheme('system', true)).toBe('dark');
  });

  it.each(['forest', 'mist', 'lavender', 'apricot'] as const)(
    'keeps the fixed %s palette light when the system is dark',
    (theme) => {
      expect(resolveAppTheme(theme, true)).toBe(theme);
    }
  );

  it('applies the resolved theme to the supplied root element', () => {
    const root = document.createElement('div');
    expect(applyAppTheme('lavender', true, root)).toBe('lavender');
    expect(root.dataset.theme).toBe('lavender');
  });
});
