export interface ModalFocusOptions {
  close: () => void;
  canClose?: boolean;
  initialFocus?: string;
}

const FOCUSABLE =
  'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';
type StackEntry = { node: HTMLElement };
const modalStack: StackEntry[] = [];

function isVisible(element: HTMLElement, root: HTMLElement) {
  let current: HTMLElement | null = element;
  while (current && root.contains(current)) {
    if (current.hidden || current.inert || current.getAttribute('aria-hidden') === 'true')
      return false;
    const style = getComputedStyle(current);
    if (
      style.display === 'none' ||
      style.visibility === 'hidden' ||
      style.visibility === 'collapse'
    )
      return false;
    current = current.parentElement;
  }
  const checkVisibility = (
    element as HTMLElement & { checkVisibility?: (options?: object) => boolean }
  ).checkVisibility;
  return (
    typeof checkVisibility !== 'function' ||
    checkVisibility.call(element, { checkOpacity: false, checkVisibilityCSS: true })
  );
}

function suppressBackground(node: HTMLElement) {
  const changed: Array<{ element: HTMLElement; inert: boolean; ariaHidden: string | null }> = [];
  let current: HTMLElement = node;
  while (current.parentElement) {
    const parent = current.parentElement;
    for (const sibling of Array.from(parent.children)) {
      if (!(sibling instanceof HTMLElement) || sibling === current) continue;
      changed.push({
        element: sibling,
        inert: sibling.inert,
        ariaHidden: sibling.getAttribute('aria-hidden')
      });
      const clickableBackdrop =
        sibling instanceof HTMLButtonElement &&
        sibling.classList.contains('fixed') &&
        sibling.classList.contains('inset-0');
      if (!clickableBackdrop) sibling.inert = true;
      sibling.setAttribute('aria-hidden', 'true');
    }
    if (parent === document.body) break;
    current = parent;
  }
  return () => {
    for (const { element, inert, ariaHidden } of changed.reverse()) {
      element.inert = inert;
      if (ariaHidden === null) element.removeAttribute('aria-hidden');
      else element.setAttribute('aria-hidden', ariaHidden);
    }
  };
}

export function modalFocus(node: HTMLElement, options: ModalFocusOptions) {
  let current = options;
  const previousFocus =
    document.activeElement instanceof HTMLElement ? document.activeElement : null;
  const entry = { node };
  modalStack.push(entry);
  const restoreBackground = suppressBackground(node);
  const isTopmost = () => modalStack.at(-1) === entry;
  const focusable = () =>
    [...node.querySelectorAll<HTMLElement>(FOCUSABLE)].filter((element) =>
      isVisible(element, node)
    );
  const focusInitial = () => {
    if (!isTopmost()) return;
    const preferred = current.initialFocus
      ? node.querySelector<HTMLElement>(current.initialFocus)
      : null;
    (
      (preferred && isVisible(preferred, node) ? preferred : null) ??
      focusable()[0] ??
      node
    ).focus();
  };
  const timer = window.setTimeout(focusInitial, 0);

  function handleKeydown(event: KeyboardEvent) {
    if (!isTopmost()) return;
    if (event.key === 'Escape' && current.canClose !== false) {
      event.preventDefault();
      event.stopPropagation();
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
    if (
      event.shiftKey &&
      (document.activeElement === first || !node.contains(document.activeElement))
    ) {
      event.preventDefault();
      last.focus();
    } else if (
      !event.shiftKey &&
      (document.activeElement === last || !node.contains(document.activeElement))
    ) {
      event.preventDefault();
      first.focus();
    }
  }

  function keepFocusInside(event: FocusEvent) {
    if (isTopmost() && event.target instanceof Node && !node.contains(event.target)) focusInitial();
  }

  document.addEventListener('keydown', handleKeydown, true);
  document.addEventListener('focusin', keepFocusInside, true);
  return {
    update(next: ModalFocusOptions) {
      current = next;
    },
    destroy() {
      window.clearTimeout(timer);
      document.removeEventListener('keydown', handleKeydown, true);
      document.removeEventListener('focusin', keepFocusInside, true);
      const index = modalStack.indexOf(entry);
      if (index >= 0) modalStack.splice(index, 1);
      restoreBackground();
      if (modalStack.length === 0) previousFocus?.focus();
      else modalStack.at(-1)?.node.focus();
    }
  };
}
