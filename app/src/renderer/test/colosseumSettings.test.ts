import { describe, expect, it } from 'vitest';
import {
  buildColosseumPhaseConfigs,
  DEFAULT_COLOSSEUM_GROOMING_SETTINGS,
  DEFAULT_COLOSSEUM_READY_SETTINGS,
  DEFAULT_COLOSSEUM_REVIEW_SETTINGS,
} from '../components/SettingsModal';

describe('Colosseum phase settings', () => {
  it('defaults each lifecycle phase to its intended persona', () => {
    const configs = buildColosseumPhaseConfigs(
      DEFAULT_COLOSSEUM_GROOMING_SETTINGS,
      { ...DEFAULT_COLOSSEUM_READY_SETTINGS, provider: 'codex', model: 'gpt-5' },
      DEFAULT_COLOSSEUM_REVIEW_SETTINGS,
    );

    expect(configs.grooming.persona).toBe('persona.architect');
    expect(configs.ready.persona).toBe('persona.engineer');
    expect(configs.review.persona).toBe('persona.reviewer');
  });

  it('normalizes tags and inherits the Ready provider when a phase omits one', () => {
    const configs = buildColosseumPhaseConfigs(
      { ...DEFAULT_COLOSSEUM_GROOMING_SETTINGS, tags: ' grooming, requirements, grooming ' },
      { ...DEFAULT_COLOSSEUM_READY_SETTINGS, provider: 'codex', model: 'gpt-5' },
      DEFAULT_COLOSSEUM_REVIEW_SETTINGS,
    );

    expect(configs.grooming.tags).toEqual(['grooming', 'requirements']);
    expect(configs.grooming.provider).toBe('codex');
    expect(configs.grooming.model).toBe('gpt-5');
    expect(configs.review.provider).toBe('codex');
    expect(configs.review.model).toBe('gpt-5');
  });

  it('keeps an explicitly configured phase provider and model', () => {
    const configs = buildColosseumPhaseConfigs(
      { ...DEFAULT_COLOSSEUM_GROOMING_SETTINGS, provider: 'claude', model: 'sonnet' },
      { ...DEFAULT_COLOSSEUM_READY_SETTINGS, provider: 'codex', model: 'gpt-5' },
      { ...DEFAULT_COLOSSEUM_REVIEW_SETTINGS, provider: 'copilot', model: 'gpt-5-mini' },
    );

    expect(configs.grooming).toMatchObject({ provider: 'claude', model: 'sonnet' });
    expect(configs.review).toMatchObject({ provider: 'copilot', model: 'gpt-5-mini' });
  });
});
