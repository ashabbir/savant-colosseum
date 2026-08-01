import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { ColosseumPhaseLedger, ColosseumRunLedger } from '../components/ColosseumTaskEvidence';

describe('Colosseum ticket evidence', () => {
  it('marks the current lifecycle phase and preserves every stage', () => {
    const html = renderToStaticMarkup(<ColosseumPhaseLedger state="review" />);

    expect(html).toContain('colosseum-phase-active');
    expect(html).toContain('review');
    expect(html).toContain('human review');
    expect(html).toContain('approved');
  });

  it('renders durable reasoning and publication evidence for a run', () => {
    const html = renderToStaticMarkup(<ColosseumRunLedger runs={[{
      run_id: 'run-1',
      phase: 'review',
      status: 'passed',
      summary: 'The implementation meets the acceptance criteria.',
      rationale: 'Focused tests and the base-to-head diff passed review.',
      questions: ['Is the rollout window confirmed?'],
      provider: 'codex',
      model: 'gpt-5',
      persona: 'persona.reviewer',
      branch: 'savant-execution/task-1',
      commit: '0123456789abcdef',
      mr_id: 'mr-colosseum-task-1',
    }]} />);

    expect(html).toContain('The implementation meets the acceptance criteria.');
    expect(html).toContain('Focused tests and the base-to-head diff passed review.');
    expect(html).toContain('Is the rollout window confirmed?');
    expect(html).toContain('persona reviewer');
    expect(html).toContain('model gpt-5');
    expect(html).toContain('commit 0123456789');
    expect(html).toContain('MR mr-colosseum-task-1');
  });
});
