import type { Workspace } from '../data';

export type WorkspaceEditorMode = 'create' | 'edit';

export type WorkspaceEditorDraft = {
  workspaceName: string;
  workspaceDescription: string;
  workspacePriority: 'critical' | 'high' | 'medium' | 'low';
  workspaceStatus: 'open' | 'closed';
  workspaceColor: string;
};

export function createWorkspaceEditorDraft(input: Partial<WorkspaceEditorDraft> = {}): WorkspaceEditorDraft {
  return {
    workspaceName: input.workspaceName ?? '',
    workspaceDescription: input.workspaceDescription ?? '',
    workspacePriority: input.workspacePriority ?? 'medium',
    workspaceStatus: input.workspaceStatus ?? 'open',
    workspaceColor: input.workspaceColor ?? '',
  };
}

export function createWorkspaceEditorDraftFromWorkspace(workspace?: Workspace, displayName?: string): WorkspaceEditorDraft {
  return createWorkspaceEditorDraft({
    workspaceName: displayName ?? workspace?.name ?? '',
    workspaceDescription: workspace?.description ?? '',
    workspacePriority: workspace?.priority ?? 'medium',
    workspaceStatus: workspace?.status === 'closed' ? 'closed' : 'open',
    workspaceColor: workspace?.color ?? '',
  });
}

export function buildWorkspaceEditorPayload(draft: WorkspaceEditorDraft) {
  return {
    name: draft.workspaceName.trim(),
    description: draft.workspaceDescription.trim(),
    priority: draft.workspacePriority,
    status: draft.workspaceStatus,
    color: draft.workspaceColor.trim() || null,
  };
}
