<script lang="ts">
  import type { ResumeColorTheme, ResumeProfile } from '$lib/types';
  import type { ResumeSectionKey, ResumeTemplateSample } from '$lib/resume-templates';
  import { formatDateRange } from '$lib/resume-format';
  import ResumeKeywordText from './ResumeKeywordText.svelte';

  export let resume: ResumeProfile | ResumeTemplateSample;
  export let sections: readonly ResumeSectionKey[];
  export let sample = false;
  export let colorTheme: ResumeColorTheme = 'navy';
  export let coverageKeywords: string[] = [];

  const themeAccents: Record<ResumeColorTheme, string> = {
    pine: '#176B57',
    navy: '#1F407A',
    graphite: '#24292F'
  };
  const themeLinks: Record<ResumeColorTheme, string> = {
    pine: '#0B7A67',
    navy: '#005CB8',
    graphite: '#24292F'
  };
  $: accent = themeAccents[colorTheme];
  $: linkColor = themeLinks[colorTheme];
  $: websiteLabel = resume.website.replace(/^https?:\/\//, '').replace(/\/$/, '');
  const degreeLabel = (education: { degree: string; degreeDetail?: string }) =>
    education.degree === '其他' ? education.degreeDetail?.trim() || '其他' : education.degree;
</script>

<article
  class="resume-paper relative mx-auto min-h-[900px] max-w-[620px] overflow-hidden bg-white px-10 py-9 text-[#111] shadow-xl"
  style={`--resume-accent: ${accent}; --resume-link: ${linkColor};`}
  data-color-theme={colorTheme}
>
  {#if sample}<div class="sample-watermark" aria-hidden="true">示例内容</div>{/if}
  <header class="relative pb-1 text-center">
    <h1 class="text-[30px] font-bold leading-none tracking-[-0.035em]">{resume.name}</h1>
    <p class="mt-2 text-[13px] font-medium leading-tight">{resume.headline}</p>
    <div class="resume-contact mt-2 flex flex-wrap justify-center text-[9px] leading-tight">
      {#if resume.location}<span>{resume.location}</span>{/if}
      {#if resume.email}<a href={`mailto:${resume.email}`}>{resume.email}</a>{/if}
      {#if resume.phone}<a href={`tel:${resume.phone}`}>{resume.phone}</a>{/if}
      {#if resume.website}<a
          href={resume.website.startsWith('http') ? resume.website : `https://${resume.website}`}
          target="_blank"
          rel="noreferrer">{websiteLabel}</a
        >{/if}
    </div>
  </header>
  {#each sections as section}
    {#if section === 'summary'}
      <section class="resume-section">
        <h2>个人简介</h2>
        <p><ResumeKeywordText text={resume.summary} highlightKeywords={coverageKeywords} /></p>
      </section>
    {:else if section === 'professionalSkills' && resume.professionalSkills.some( (group) => group.items.some( (item) => item.trim() ) )}
      <section class="resume-section">
        <h2>专业技能</h2>
        {#each resume.professionalSkills.filter( (group) => group.items.some( (item) => item.trim() ) ) as group}<p
            class="mt-1"
          >
            <strong>{group.label || '专业技能'}：</strong><ResumeKeywordText
              text={group.items
                .map((item) => item.trim())
                .filter(Boolean)
                .join(', ')}
              highlightKeywords={coverageKeywords}
            />
          </p>{/each}
      </section>
    {:else if section === 'experiences'}
      <section class="resume-section">
        <h2>工作经历</h2>
        {#each resume.experiences as experience}<div class="mb-4">
            <div class="flex items-baseline justify-between gap-3">
              <span
                ><strong>{experience.company}</strong>{experience.position
                  ? `，${experience.position}`
                  : ''}</span
              ><span
                >{experience.location ? `${experience.location} · ` : ''}{formatDateRange(
                  experience.startDate,
                  experience.endDate
                )}</span
              >
            </div>
            {#if experience.highlights.some((item) => item.trim())}<ul>
                {#each experience.highlights.filter((item) => item.trim()) as highlight}<li>
                    <ResumeKeywordText text={highlight} highlightKeywords={coverageKeywords} />
                  </li>{/each}
              </ul>{/if}
          </div>{/each}
      </section>
    {:else if section === 'projects' && resume.projects.length}
      <section class="resume-section">
        <h2>项目经历</h2>
        {#each resume.projects as project}<div class="mb-4">
            <div class="flex items-baseline justify-between gap-3">
              <strong>{project.name}</strong><span
                >{formatDateRange(project.startDate, project.endDate)}</span
              >
            </div>
            <p><ResumeKeywordText text={project.summary} highlightKeywords={coverageKeywords} /></p>
            {#if project.highlights.some((item) => item.trim())}<ul>
                {#each project.highlights.filter((item) => item.trim()) as highlight}<li>
                    <ResumeKeywordText text={highlight} highlightKeywords={coverageKeywords} />
                  </li>{/each}
              </ul>{/if}
          </div>{/each}
      </section>
    {:else if section === 'certifications' && resume.certifications.length}
      <section class="resume-section">
        <h2>证书 / 专业资质</h2>
        {#each resume.certifications as certification}<p>
            <strong>{certification.name}</strong>{certification.issuer
              ? ` · ${certification.issuer}`
              : ''}{certification.date ? ` · ${certification.date}` : ''}
          </p>{/each}
      </section>
    {:else if section === 'education'}
      <section class="resume-section">
        <h2>教育经历</h2>
        {#each resume.education as education}<div class="mb-3">
            <div class="flex items-baseline justify-between gap-3">
              <strong>{education.institution} · {education.area}</strong><span
                >{formatDateRange(education.startDate, education.endDate)}</span
              >
            </div>
            <p>{degreeLabel(education)}</p>
            {#if education.highlights.some((item) => item.trim())}<ul>
                {#each education.highlights.filter((item) => item.trim()) as highlight}<li>
                    <ResumeKeywordText text={highlight} highlightKeywords={coverageKeywords} />
                  </li>{/each}
              </ul>{/if}
          </div>{/each}
      </section>
    {/if}
  {/each}
</article>

<style>
  .resume-paper {
    font-family: 'Microsoft YaHei', 'PingFang SC', sans-serif;
  }
  .resume-paper header h1,
  .resume-paper header p {
    color: var(--resume-accent);
  }
  .resume-contact {
    color: var(--resume-accent);
  }
  .resume-contact > :not(:last-child)::after {
    margin: 0 7px;
    content: '|';
    color: var(--resume-accent);
  }
  .resume-section {
    position: relative;
    margin-top: 16px;
    font-size: 10.2px;
    line-height: 1.58;
  }
  .resume-section h2 {
    margin-bottom: 7px;
    border-bottom: 1px solid var(--resume-accent);
    padding-bottom: 2px;
    color: var(--resume-accent);
    font-size: 13px;
    font-weight: 700;
    letter-spacing: 0.01em;
  }
  .resume-section ul {
    margin-top: 5px;
    list-style: disc;
    padding-left: 15px;
  }
  .resume-section li {
    margin-top: 2px;
  }
  .resume-paper a {
    color: var(--resume-link);
  }
  .sample-watermark {
    position: absolute;
    left: 50%;
    top: 48%;
    transform: translate(-50%, -50%) rotate(-24deg);
    color: color-mix(in srgb, var(--resume-accent) 8%, transparent);
    font-size: 72px;
    font-weight: 800;
    letter-spacing: 0.12em;
    white-space: nowrap;
  }
</style>
