import { useEffect, useMemo, useRef, useState, type FormEvent } from 'react';
import { Copy, Loader2, Send, Sparkles, Trash2, X } from 'lucide-react';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import type { Artifact, Session, Task, Workspace } from '../data';
import type { ChatMessage } from '../services/athenaService';

type WorkspaceAthenaDrawerProps = {
  isOpen: boolean;
  onClose: () => void;
  activeWorkspace?: Workspace;
  activeWorkspaceId: string;
  messages: ChatMessage[];
  isLoading: boolean;
  error: string;
  persistedInput: string;
  onSendMessage: (text: string) => void;
  onDeleteMessage: (id: string) => void;
  onClearChat?: () => void;
  onChangeInput: (text: string) => void;
  workspaceSessions: Session[];
  workspaceTasks: Task[];
  workspaceNotes: { id: string; sessionId: string; title: string; body: string; createdAt?: string }[];
  workspaceArtifacts: Artifact[];
  workspaceMergeRequests: { id: string; title: string; status: string; mrId: string; createdAt?: string; updatedAt?: string }[];
  workspaceJiraTickets: { id: string; title: string; status: string; ticketKey: string; createdAt?: string; updatedAt?: string }[];
  workspaceActivitySummary: { total: number; detail: string; latest: string };
  selectedProvider?: string;
  selectedModel?: string;
  athenaType?: 'workspace' | 'task_manager';
};

export function WorkspaceAthenaDrawer({
  isOpen,
  onClose,
  activeWorkspace,
  activeWorkspaceId,
  messages,
  isLoading,
  error,
  persistedInput,
  onSendMessage,
  onDeleteMessage,
  onClearChat,
  onChangeInput,
  workspaceSessions,
  workspaceTasks,
  workspaceNotes,
  workspaceArtifacts,
  workspaceMergeRequests,
  workspaceJiraTickets,
  workspaceActivitySummary,
  selectedProvider = 'gemini',
  selectedModel = '3.5',
  athenaType = 'workspace',
}: WorkspaceAthenaDrawerProps) {
  const [input, setInput] = useState(persistedInput || '');
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    setInput(persistedInput || '');
  }, [activeWorkspaceId, persistedInput]);

  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        onClose();
      }
    };
    if (isOpen) {
      window.addEventListener('keydown', handleKeyDown);
      // Auto-focus input when opening
      window.setTimeout(() => inputRef.current?.focus(), 50);
    }
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isOpen, onClose]);

  const contextStats = useMemo(() => {
    if (athenaType === 'task_manager') {
      return [
        { label: 'tasks (global)', value: workspaceTasks.length },
      ];
    }
    return [
      { label: 'sessions', value: workspaceSessions.length },
      { label: 'tasks', value: workspaceTasks.length },
      { label: 'notes', value: workspaceNotes.length },
      { label: 'activity', value: workspaceActivitySummary.total },
    ];
  }, [athenaType, workspaceActivitySummary.total, workspaceNotes.length, workspaceSessions.length, workspaceTasks.length]);

  if (!isOpen) return null;

  const sendMessage = (event?: FormEvent) => {
    event?.preventDefault();
    const userText = input.trim();
    if (!userText) return;
    onSendMessage(userText);
    setInput('');
  };

  const handleInputChange = (val: string) => {
    setInput(val);
    onChangeInput(val);
  };

  const copyMessage = (text: string) => {
    void navigator.clipboard?.writeText(text);
  };

  return (
    <div className="activity-drawer-backdrop" onClick={onClose}>
      <aside className="workspace-athena-drawer" onClick={(event) => event.stopPropagation()} aria-label="Ask Athena">
        <header className="workspace-athena-header">
          <div>
            <div className="eyebrow">ATHENA</div>
            <h2>{athenaType === 'task_manager' ? 'Task Manager Context' : 'Workspace Context'}</h2>
          </div>
          <div className="flex items-center gap-2">
            <button
              type="button"
              className="px-2 py-0.5 border border-cyan-500/30 text-cyan-400 bg-cyan-950/20 text-[10px] font-mono tracking-wider rounded uppercase hover:bg-cyan-900/30 transition-all cursor-pointer whitespace-nowrap"
              title={`Active Model: ${selectedProvider} (${selectedModel})`}
            >
              {selectedProvider}:{selectedModel}
            </button>
            <button className="icon-btn" type="button" onClick={onClose} title="Close Athena" aria-label="Close Athena">
              <X size={16} />
            </button>
          </div>
        </header>

        <div className="workspace-athena-context-strip">
          {contextStats.map((item) => (
            <div key={item.label}>
              <span>{item.label}</span>
              <strong>{item.value}</strong>
            </div>
          ))}
        </div>

        <div className="workspace-athena-messages">
          {messages.length === 0 ? (
            <div className="workspace-athena-empty">
              <Sparkles size={28} />
              <span>Ask about the current workspace graph, sessions, tasks, notes, Jira, merge requests, or artifacts.</span>
            </div>
          ) : messages.map((message) => (
            <article key={message.id} className={`workspace-athena-message workspace-athena-message-${message.sender}`}>
              <div className="workspace-athena-message-body">
                {message.sender === 'assistant' ? (
                  <ReactMarkdown remarkPlugins={[remarkGfm]}>{message.text}</ReactMarkdown>
                ) : message.text}
              </div>
              <footer>
                <span>{message.sender === 'user' ? 'Operator' : 'Athena'} · {new Date(message.timestamp).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}</span>
                <div>
                  <button type="button" onClick={() => copyMessage(message.text)} title="Copy message" aria-label="Copy message"><Copy size={12} /></button>
                  <button type="button" onClick={() => onDeleteMessage(message.id)} title="Delete message" aria-label="Delete message"><Trash2 size={12} /></button>
                </div>
              </footer>
            </article>
          ))}
          {isLoading && (
            <div className="workspace-athena-loading">
              <Loader2 size={14} />
              <span>Reading workspace context...</span>
            </div>
          )}
          {error && <div className="workspace-athena-error">{error}</div>}
        </div>

        <form className="workspace-athena-composer" onSubmit={sendMessage}>
          <input
            ref={inputRef}
            type="text"
            value={input}
            onChange={(event) => handleInputChange(event.target.value)}
            placeholder={isLoading ? "Athena is thinking... send message to steer" : "Ask Athena about this workspace..."}
          />
          <button type="submit" disabled={!input.trim()} title="Send" aria-label="Send">
            <Send size={15} />
          </button>
          {messages.length > 0 && onClearChat && (
            <button
              type="button"
              onClick={() => { if (window.confirm('Clear all Athena chat history for this workspace?')) onClearChat(); }}
              title="Clear chat history"
              aria-label="Clear chat"
              className="ghost-btn action-close icon-only"
              style={{ opacity: 0.5 }}
            >
              <Trash2 size={13} />
            </button>
          )}
        </form>
      </aside>
    </div>
  );
}

function formatHistory(messages: ChatMessage[]) {
  if (!messages.length) return 'No previous messages in this workspace Athena chat.';
  return messages.slice(-12).map((message) => `${message.sender === 'user' ? 'User' : 'Athena'}: ${message.text}`).join('\n');
}
