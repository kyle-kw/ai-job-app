import type { ResumeDegree, ResumeEducation } from '$lib/types';

export function formatDateRange(startDate?: string, endDate?: string): string {
  const start = (startDate ?? '').trim().replace(/^[-–—\s]+|[-–—\s]+$/g, '');
  const end = (endDate ?? '').trim().replace(/^[-–—\s]+|[-–—\s]+$/g, '');
  if (start && end) return `${start}—${end}`;
  return start || end;
}

export function displayDegree(education: Pick<ResumeEducation, 'degree' | 'degreeDetail'>): string {
  return education.degree === '其他' ? education.degreeDetail?.trim() || '其他' : education.degree;
}

export function normalizeDegree(
  value: string,
  detail = ''
): { degree: ResumeDegree; degreeDetail: string } {
  const raw = value.trim();
  if (!raw) return { degree: '', degreeDetail: detail.trim() };
  if (raw.includes('博士')) return { degree: '博士', degreeDetail: '' };
  if (raw.includes('硕士')) return { degree: '硕士', degreeDetail: '' };
  if (raw.includes('本科') || raw.includes('学士')) return { degree: '本科', degreeDetail: '' };
  if (raw === '其他') return { degree: '其他', degreeDetail: detail.trim() };
  return { degree: '其他', degreeDetail: detail.trim() || raw };
}

export function safeResumeFileName(name: string, date = new Date()): string {
  const safeName = (name.trim() || '未命名简历').replace(/[\\/:*?"<>|]/g, '_');
  const pad = (value: number) => String(value).padStart(2, '0');
  const timestamp = `${date.getFullYear()}${pad(date.getMonth() + 1)}${pad(date.getDate())}-${pad(date.getHours())}${pad(date.getMinutes())}${pad(date.getSeconds())}`;
  return `${safeName}-${timestamp}.pdf`;
}
