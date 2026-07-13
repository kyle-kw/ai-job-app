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
});
