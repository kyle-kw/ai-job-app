import { derived, writable } from 'svelte/store';

export type Locale = 'zh-CN' | 'en';

const messages: Record<Locale, Record<string, string>> = {
  'zh-CN': {
    'nav.dashboard': '初始化',
    'nav.jobs': '岗位',
    'nav.reports': '数据报告',
    'nav.resume': '简历',
    'nav.settings': '设置',
    'nav.tasks': '任务中心',
    'app.tagline': '把求职流程变得清楚一点',
    'common.ready': '已就绪',
    'common.pending': '待完成',
    'common.save': '保存',
    'common.cancel': '取消'
  },
  en: {
    'nav.dashboard': 'Setup',
    'nav.jobs': 'Jobs',
    'nav.reports': 'Reports',
    'nav.resume': 'Resume',
    'nav.settings': 'Settings',
    'nav.tasks': 'Tasks',
    'app.tagline': 'A calmer, clearer job search',
    'common.ready': 'Ready',
    'common.pending': 'Not ready',
    'common.save': 'Save',
    'common.cancel': 'Cancel'
  }
};

export const locale = writable<Locale>('zh-CN');
export const t = derived(locale, () => (key: string) => messages['zh-CN'][key] ?? key);
