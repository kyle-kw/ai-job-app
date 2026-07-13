export const MAX_RESUME_FILE_BYTES = 25 * 1024 * 1024;
export const RESUME_FILE_EXTENSIONS = ['pdf', 'docx', 'yaml', 'yml'] as const;

export function validateResumeFile(file: Pick<File, 'name' | 'size'>): void {
  const extension = file.name.split('.').pop()?.toLocaleLowerCase() ?? '';
  if (!RESUME_FILE_EXTENSIONS.includes(extension as (typeof RESUME_FILE_EXTENSIONS)[number])) {
    throw new Error('仅支持 PDF、DOCX、YAML 和 YML 文件。');
  }
  if (file.size > MAX_RESUME_FILE_BYTES) {
    throw new Error('简历文件不能超过 25 MiB。');
  }
}

export function readResumeAsBase64(file: File): Promise<string> {
  validateResumeFile(file);
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () => reject(new Error('无法读取简历文件，请重新选择。'));
    reader.onload = () => {
      const result = typeof reader.result === 'string' ? reader.result : '';
      const separator = result.indexOf(',');
      if (separator < 0) reject(new Error('简历文件编码失败，请重新选择。'));
      else resolve(result.slice(separator + 1));
    };
    reader.readAsDataURL(file);
  });
}
