import { downloadDir, join } from '@tauri-apps/api/path';
import { save as saveDialog } from '@tauri-apps/plugin-dialog';

export function localExportStamp(date = new Date()): string {
  const part = (value: number) => String(value).padStart(2, '0');
  return `${date.getFullYear()}${part(date.getMonth() + 1)}${part(date.getDate())}_${part(date.getHours())}${part(date.getMinutes())}${part(date.getSeconds())}`;
}

export async function chooseLocalExportPath(options: {
  title: string;
  fileName: string;
  filterName: string;
  extension: string;
}): Promise<string | null> {
  if (typeof window === 'undefined' || !window.__TAURI_INTERNALS__) return options.fileName;
  return saveDialog({
    title: options.title,
    defaultPath: await join(await downloadDir(), options.fileName),
    filters: [{ name: options.filterName, extensions: [options.extension] }]
  });
}
