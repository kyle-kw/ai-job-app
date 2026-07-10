import { derived, writable } from 'svelte/store';

export type Locale = 'zh-CN' | 'en';

const messages: Record<Locale, Record<string, string>> = {
  'zh-CN': {
    'nav.dashboard': '工作台',
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
    'nav.dashboard': 'Workspace',
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

const initial = typeof localStorage === 'undefined' ? 'zh-CN' : (localStorage.getItem('locale') as Locale) || 'zh-CN';
export const locale = writable<Locale>(initial);
locale.subscribe((value) => {
  if (typeof localStorage !== 'undefined') localStorage.setItem('locale', value);
});

export const t = derived(locale, ($locale) => (key: string) => messages[$locale][key] ?? key);
