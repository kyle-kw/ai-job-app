import { describe, expect, it, vi } from 'vitest';
import { modalFocus } from './modal-focus';

describe('modalFocus', () => {
  it('moves, traps, closes, and restores focus', async () => {
    const trigger = document.body.appendChild(document.createElement('button'));
    trigger.focus();
    const dialog = document.body.appendChild(document.createElement('div'));
    dialog.tabIndex = -1;
    const first = dialog.appendChild(document.createElement('button'));
    const last = dialog.appendChild(document.createElement('button'));
    const close = vi.fn();
    const action = modalFocus(dialog, { close });
    await new Promise((resolve) => window.setTimeout(resolve, 0));
    expect(document.activeElement).toBe(first);

    last.focus();
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Tab', bubbles: true }));
    expect(document.activeElement).toBe(first);
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    expect(close).toHaveBeenCalledOnce();

    action.destroy();
    expect(document.activeElement).toBe(trigger);
    dialog.remove();
    trigger.remove();
  });

  it('ignores hidden controls, suppresses background, and only closes the top modal', async () => {
    const background = document.body.appendChild(document.createElement('main'));
    const outer = document.body.appendChild(document.createElement('div'));
    outer.tabIndex = -1;
    const hidden = outer.appendChild(document.createElement('button'));
    hidden.style.display = 'none';
    const visible = outer.appendChild(document.createElement('button'));
    const closeOuter = vi.fn();
    const outerAction = modalFocus(outer, { close: closeOuter });
    await new Promise((resolve) => window.setTimeout(resolve, 0));
    expect(document.activeElement).toBe(visible);
    expect(background.inert).toBe(true);
    expect(background).toHaveAttribute('aria-hidden', 'true');

    const inner = outer.appendChild(document.createElement('div'));
    inner.tabIndex = -1;
    inner.appendChild(document.createElement('button'));
    const closeInner = vi.fn();
    const innerAction = modalFocus(inner, { close: closeInner });
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }));
    expect(closeInner).toHaveBeenCalledOnce();
    expect(closeOuter).not.toHaveBeenCalled();

    innerAction.destroy();
    outerAction.destroy();
    expect(background.inert).not.toBe(true);
    expect(background).not.toHaveAttribute('aria-hidden');
    inner.remove();
    outer.remove();
    background.remove();
  });
});
