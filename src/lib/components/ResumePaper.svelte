<script lang="ts">
  import { Mail, MapPin, Phone } from 'lucide-svelte';
  import type { ResumeProfile } from '$lib/types';
  import type { ResumeSectionKey, ResumeTemplateSample } from '$lib/resume-templates';
  import { formatDateRange } from '$lib/resume-format';

  export let resume: ResumeProfile | ResumeTemplateSample;
  export let sections: readonly ResumeSectionKey[];
  export let sample = false;

  $: themeClass = resume.templateId === 'data-analysis' ? 'theme-data' : resume.templateId === 'finance-accounting' ? 'theme-finance' : 'theme-it';
  const degreeLabel = (education: { degree: string; degreeDetail?: string }) => education.degree === '其他' ? education.degreeDetail?.trim() || '其他' : education.degree;
</script>

<article class={`resume-paper ${themeClass} relative mx-auto min-h-[900px] max-w-[620px] overflow-hidden bg-white px-12 py-11 text-[#17201d] shadow-xl`}>
  {#if sample}<div class="sample-watermark" aria-hidden="true">示例内容</div>{/if}
  <header class="relative border-b-2 border-[#176b57] pb-5">
    <h1 class="text-[32px] font-bold tracking-[-0.04em] text-[#176b57]">{resume.name}</h1>
    <p class="mt-1 text-[15px] font-semibold">{resume.headline}</p>
    <div class="mt-3 flex flex-wrap gap-x-4 gap-y-1 text-[9px] text-[#5c6863]">
      <span class="flex items-center gap-1"><Mail size={10} />{resume.email}</span>
      <span class="flex items-center gap-1"><Phone size={10} />{resume.phone}</span>
      <span class="flex items-center gap-1"><MapPin size={10} />{resume.location}</span>
    </div>
  </header>
  {#each sections as section}
    {#if section === 'summary'}
      <section class="resume-section"><h2>个人简介</h2><p>{resume.summary}</p></section>
    {:else if section === 'professionalSkills'}
      <section class="resume-section"><h2>专业技能</h2>{#each resume.professionalSkills as group}<p class="mt-1"><strong>{group.label}：</strong>{group.items.filter(Boolean).join('、')}</p>{/each}</section>
    {:else if section === 'experiences'}
      <section class="resume-section"><h2>工作经历</h2>{#each resume.experiences as experience}<div class="mb-4"><div class="flex items-baseline justify-between gap-3"><strong>{experience.position} · {experience.company}</strong><span>{formatDateRange(experience.startDate, experience.endDate)}</span></div><ul>{#each experience.highlights as highlight}<li>{highlight}</li>{/each}</ul></div>{/each}</section>
    {:else if section === 'projects' && resume.projects.length}
      <section class="resume-section"><h2>项目经历</h2>{#each resume.projects as project}<div class="mb-4"><div class="flex items-baseline justify-between gap-3"><strong>{project.name}</strong><span>{formatDateRange(project.startDate, project.endDate)}</span></div><p>{project.summary}</p><ul>{#each project.highlights as highlight}<li>{highlight}</li>{/each}</ul></div>{/each}</section>
    {:else if section === 'certifications' && resume.certifications.length}
      <section class="resume-section"><h2>证书 / 专业资质</h2>{#each resume.certifications as certification}<p><strong>{certification.name}</strong>{certification.issuer ? ` · ${certification.issuer}` : ''}{certification.date ? ` · ${certification.date}` : ''}</p>{/each}</section>
    {:else if section === 'education'}
      <section class="resume-section"><h2>教育经历</h2>{#each resume.education as education}<div class="flex items-baseline justify-between gap-3"><strong>{education.institution} · {education.area}</strong><span>{formatDateRange(education.startDate, education.endDate)}</span></div><p>{degreeLabel(education)}</p>{/each}</section>
    {/if}
  {/each}
</article>

<style>
  .resume-paper { font-family: "Source Sans 3", "PingFang SC", sans-serif; }
  .resume-section { position: relative; margin-top: 20px; font-size: 10px; line-height: 1.55; }
  .resume-section h2 { margin-bottom: 8px; border-bottom: 1px solid #aab7b1; padding-bottom: 3px; color: #176b57; font-size: 13px; font-weight: 700; text-transform: uppercase; letter-spacing: .04em; }
  .resume-section ul { margin-top: 5px; list-style: disc; padding-left: 16px; }
  .resume-section li { margin-top: 2px; }
  .theme-it { font-family: "XCharter", "Source Sans 3", "PingFang SC", sans-serif; }
  .theme-it header { border-color: #111; }
  .theme-it header h1, .theme-it .resume-section h2 { color: #111; }
  .theme-data { font-family: "Lato", "Source Sans 3", "PingFang SC", sans-serif; }
  .theme-data header { border-color: #00645a; text-align: center; }
  .theme-data header h1, .theme-data .resume-section h2 { color: #00645a; }
  .theme-data header div { justify-content: center; }
  .theme-data .resume-section h2 { border-bottom: 0; text-align: center; letter-spacing: .08em; }
  .theme-finance { font-family: "XCharter", "Source Sans 3", "Songti SC", serif; }
  .theme-finance header { border-color: #222; text-align: center; }
  .theme-finance header h1, .theme-finance .resume-section h2 { color: #111; }
  .theme-finance header div { justify-content: center; }
  .theme-finance .resume-section h2 { border-bottom: 0; text-align: center; }
  .sample-watermark { position: absolute; left: 50%; top: 48%; transform: translate(-50%, -50%) rotate(-24deg); color: rgba(23, 107, 87, .08); font-size: 72px; font-weight: 800; letter-spacing: .12em; white-space: nowrap; }
</style>
