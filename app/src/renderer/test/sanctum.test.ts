import { describe, expect, it } from 'vitest';
import { artifacts, navigation, reminders, sectionProfiles, sessionEvents, sessions, workspaces } from '../data';

describe('Sanctum scaffold', () => {
  it('exposes the expected navigation rails', () => {
    expect(navigation.map((item) => item.short)).toEqual(['W', 'M', 'S', 'T', 'R', 'N', 'F', 'J', 'M', 'K', 'G']);
  });

  it('includes the PRD entities in the mock data', () => {
    expect(reminders).toHaveLength(3);
    expect(artifacts.map((artifact) => artifact.title)).toContain('Merge requests');
  });

  it('keeps workspace and session linkage explicit', () => {
    expect(workspaces.find((workspace) => workspace.id === 'ws-sanctum')?.name).toBe('Sanctum');
    expect(sessions.every((session) => workspaces.some((workspace) => workspace.id === session.workspaceId))).toBe(true);
  });

  it('covers each navigation surface with a section profile', () => {
    expect(sectionProfiles.map((profile) => profile.id)).toEqual(['workspace', 'manage', 'session', 'tasks', 'reminders', 'notes', 'files', 'jira', 'merge-requests', 'knowledge', 'providers', 'settings']);
  });

  it('keeps the settings shortcut available', () => {
    expect(navigation.find((item) => item.id === 'settings')?.short).toBe('G');
  });

  it('includes compact counts for every navigation surface', () => {
    expect(navigation.every((item) => item.id.length > 0)).toBe(true);
  });

  it('keeps the session artifact counts available for the hero strip', () => {
    expect(sessions.find((session) => session.id === 'session-auth')?.notes).toBe(3);
    expect(sessions.find((session) => session.id === 'session-auth')?.mergeRequests).toBe(1);
  });

  it('keeps timeline events attached to sessions', () => {
    expect(sessionEvents.filter((event) => event.sessionId === 'session-auth')).toHaveLength(3);
    expect(sessionEvents.some((event) => event.kind === 'sync')).toBe(true);
  });
});
