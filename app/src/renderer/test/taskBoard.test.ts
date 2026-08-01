import { describe, expect, it } from 'vitest';
import type { Task } from '../data';
import { COLOSSEUM_PHASES, canMoveTask, canSubmitForGrooming, isAwaitingHumanReview, isTaskBlocked } from '../lib/taskBoard';

const mockTask: Task = {
  id: 'task-100',
  workspaceId: 'ws-sanctum',
  title: 'Test status and blocking',
  priority: 'medium',
  state: 'backlog',
  owner: 'ahmed',
};

describe('Task status and blocking rules', () => {
  it('identifies blocked tasks from taskFlags', () => {
    expect(isTaskBlocked(mockTask, {})).toBe(false);
    expect(isTaskBlocked(mockTask, { 'task-100': { blocked: true } })).toBe(true);
  });

  it('prevents changing status when task is blocked', () => {
    const unblockedCanMove = canMoveTask(mockTask, 'in-progress', {});
    expect(unblockedCanMove).toBe(true);

    const blockedCanMove = canMoveTask(mockTask, 'in-progress', { 'task-100': { blocked: true } });
    expect(blockedCanMove).toBe(false);
  });

  it('allows same status when task is blocked', () => {
    const sameStatusMove = canMoveTask(mockTask, 'backlog', { 'task-100': { blocked: true } });
    expect(sameStatusMove).toBe(true);
  });

  it('defines the complete staged Colosseum lifecycle', () => {
    expect(COLOSSEUM_PHASES).toEqual([
      'backlog',
      'grooming',
      'ready',
      'in-progress',
      'review',
      'human-review',
      'approved',
      'done',
    ]);
  });

  it('requires a repository before submitting development work for grooming', () => {
    expect(canSubmitForGrooming({
      ...mockTask,
      colosseumConfig: { work_type: 'development' },
    })).toBe(false);
    expect(canSubmitForGrooming({
      ...mockTask,
      colosseumConfig: { work_type: 'development', repository: '/repo' },
    })).toBe(true);
    expect(canSubmitForGrooming({
      ...mockTask,
      colosseumConfig: { work_type: 'research' },
    })).toBe(true);
    expect(canSubmitForGrooming({
      ...mockTask,
      state: 'grooming',
      colosseumConfig: { work_type: 'research' },
    })).toBe(true);
  });

  it('recognizes only the human-review gate as awaiting operator approval', () => {
    expect(isAwaitingHumanReview({ ...mockTask, state: 'human-review' })).toBe(true);
    expect(isAwaitingHumanReview({ ...mockTask, state: 'review' })).toBe(false);
  });
});
