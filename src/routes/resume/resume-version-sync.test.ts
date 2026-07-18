import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/svelte';
import { afterEach, describe, expect, it } from 'vitest';
import { mockSnapshot } from '$lib/mock-data';
import { snapshot } from '$lib/stores/app';
import ResumePage from './+page.svelte';

afterEach(cleanup);

describe('resume version synchronization', () => {
  it('adopts a newer version with the same resume id when the draft is clean', async () => {
    snapshot.set(structuredClone(mockSnapshot));
    render(ResumePage);

    snapshot.update((state) => ({
      ...state,
      resume: state.resume
        ? { ...state.resume, version: state.resume.version + 1, headline: '后台导入的新标题' }
        : null
    }));

    await waitFor(() => expect(screen.getByLabelText('职业标题')).toHaveValue('后台导入的新标题'));
    expect(screen.getByRole('button', { name: '已保存' })).toBeDisabled();
  });

  it('preserves local edits and blocks stale saves when a newer version arrives', async () => {
    snapshot.set(structuredClone(mockSnapshot));
    render(ResumePage);
    const headline = screen.getByLabelText('职业标题');
    await fireEvent.input(headline, { target: { value: '尚未保存的本地标题' } });

    snapshot.update((state) => ({
      ...state,
      resume: state.resume
        ? { ...state.resume, version: state.resume.version + 1, headline: '后台导入的新标题' }
        : null
    }));

    expect(headline).toHaveValue('尚未保存的本地标题');
    expect(await screen.findByRole('alert')).toHaveTextContent('检测到新的简历版本');
    expect(screen.getByRole('button', { name: '保存修改' })).toBeDisabled();

    await fireEvent.click(screen.getByRole('button', { name: '载入最新版本' }));
    await waitFor(() => expect(screen.getByLabelText('职业标题')).toHaveValue('后台导入的新标题'));
  });
});
