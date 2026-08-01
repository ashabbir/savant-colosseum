import type { SessionConversationMessage } from '../services/sessionAdapters';

type SessionConversationProps = {
  messages: SessionConversationMessage[];
};

export function SessionConversation({ messages }: SessionConversationProps) {
  return (
    <div className="chat-transcript">
      {messages.length === 0 ? (
        <div className="activity-empty">No chats in this session.</div>
      ) : (
        messages.map((message) => (
          <article key={message.id} className={`chat-transcript-item chat-transcript-${message.kind}`}>
            <div className="chat-transcript-head">
              <span className="chat-transcript-role">{message.title}</span>
              <span className="chat-transcript-time">{message.time || message.provider}</span>
            </div>
            <div className="chat-transcript-title">{message.role}</div>
            <p className="chat-transcript-detail">{message.detail}</p>
          </article>
        ))
      )}
    </div>
  );
}
