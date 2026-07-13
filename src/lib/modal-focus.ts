export interface ModalFocusOptions {
  close: () => void;
  canClose?: boolean;
  initialFocus?: string;
}

const FOCUSABLE = 'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

export function modalFocus(node: HTMLElement, options: ModalFocusOptions) {
  let current = options;
  const previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;

  const focusable = () => [...node.querySelectorAll<HTMLElement>(FOCUSABLE)].filter((element) => !element.hidden && element.getAttribute('aria-hidden') !== 'true');
  const focusInitial = () => {
    const preferred = current.initialFocus ? node.querySelector<HTMLElement>(current.initialFocus) : null;
    (preferred ?? focusable()[0] ?? node).focus();
  };
  const timer = window.setTimeout(focusInitial, 0);

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape' && current.canClose !== false) {
      event.preventDefault();
      current.close();
      return;
    }
    if (event.key !== 'Tab') return;
    const elements = focusable();
    if (!elements.length) {
      event.preventDefault();
      node.focus();
      return;
    }
    const first = elements[0];
    const last = elements[elements.length - 1];
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  }

  function keepFocusInside(event: FocusEvent) {
    if (event.target instanceof Node && !node.contains(event.target)) focusInitial();
  }

  document.addEventListener('keydown', handleKeydown, true);
  document.addEventListener('focusin', keepFocusInside, true);
  return {
    update(next: ModalFocusOptions) { current = next; },
    destroy() {
      window.clearTimeout(timer);
      document.removeEventListener('keydown', handleKeydown, true);
      document.removeEventListener('focusin', keepFocusInside, true);
      previousFocus?.focus();
    }
  };
}
