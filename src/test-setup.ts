import '@testing-library/jest-dom/vitest';
import { afterEach } from 'vitest';
import { resetBrowserMockState } from '$lib/services/backend';

afterEach(() => {
  resetBrowserMockState();
});
