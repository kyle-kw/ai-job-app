import { describe, expect, it } from 'vitest';
import { MAX_RESUME_FILE_BYTES, readResumeAsBase64, validateResumeFile } from './resume-import';

describe('resume import validation', () => {
  it('accepts supported files through the 25 MiB boundary', () => {
    expect(() => validateResumeFile({ name: 'resume.PDF', size: MAX_RESUME_FILE_BYTES } as File)).not.toThrow();
  });

  it('rejects oversized and unsupported files', () => {
    expect(() => validateResumeFile({ name: 'resume.pdf', size: MAX_RESUME_FILE_BYTES + 1 } as File)).toThrow('25 MiB');
    expect(() => validateResumeFile({ name: 'resume.txt', size: 10 } as File)).toThrow('仅支持');
  });

  it('encodes a supported file without byte-by-byte string concatenation', async () => {
    const file = new File(['abc'], 'resume.yaml', { type: 'text/yaml' });
    await expect(readResumeAsBase64(file)).resolves.toBe('YWJj');
  });
});
