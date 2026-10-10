import '@testing-library/jest-dom/vitest';
import { afterEach } from 'vitest';
import { clearJobsBrowseSnapshot } from '$lib/jobs-browse-cache';
import { resetBrowserMockState } from '$lib/services/backend';

afterEach(() => {
  resetBrowserMockState();
  clearJobsBrowseSnapshot();
});
