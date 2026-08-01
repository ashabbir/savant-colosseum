import { describe, expect, it } from 'vitest';
import { getSessionAdapter } from '../services/sessionAdapters';

describe('session adapters', () => {
  it('parses codex transcript jsonl into chat messages', () => {
    const adapter = getSessionAdapter('codex');
    const transcript = [
      JSON.stringify({ type: 'message', role: 'user', content: [{ type: 'input_text', text: 'Build the feature' }] }),
      JSON.stringify({
        type: 'message',
        role: 'assistant',
        content: [{ type: 'output_text', text: 'Working on it' }],
        tool_calls: [{ name: 'shell' }],
      }),
    ].join('\n');

    const messages = adapter.normalizeConversationText(transcript, 'codex-1');
    expect(messages).toHaveLength(3);
    expect(messages[0]).toMatchObject({ kind: 'user', role: 'user', detail: 'Build the feature', provider: 'codex' });
    expect(messages[1]).toMatchObject({ kind: 'assistant', role: 'assistant', provider: 'codex' });
    expect(messages[2]).toMatchObject({ kind: 'tool', title: 'shell', provider: 'codex' });
  });

  it('parses copilot transcript jsonl into chat messages', () => {
    const adapter = getSessionAdapter('copilot');
    const transcript = [
      JSON.stringify({ type: 'user.message', data: { content: 'hello from copilot' } }),
      JSON.stringify({ type: 'assistant.message', data: { content: 'copilot reply', toolRequests: [{ toolName: 'terminal' }] } }),
    ].join('\n');

    const messages = adapter.normalizeConversationText(transcript, 'copilot-1');
    expect(messages).toHaveLength(3);
    expect(messages[0]).toMatchObject({ kind: 'user', detail: 'hello from copilot', provider: 'copilot' });
    expect(messages[2]).toMatchObject({ kind: 'tool', title: 'terminal' });
  });

  it('parses claude transcript json into chat messages', () => {
    const adapter = getSessionAdapter('claude');
    const transcript = JSON.stringify({
      messages: [
        { type: 'user', content: [{ text: 'claude prompt' }] },
        { type: 'assistant', content: 'claude response', toolCalls: [{ name: 'read_file', displayName: 'Read File' }] },
      ],
    });

    const messages = adapter.normalizeConversationText(transcript, 'claude-1');
    expect(messages).toHaveLength(3);
    expect(messages[0]).toMatchObject({ kind: 'user', role: 'user', detail: 'claude prompt', provider: 'claude' });
    expect(messages[2]).toMatchObject({ kind: 'tool', title: 'Read File' });
  });

  it('parses gemini transcript json into chat messages', () => {
    const adapter = getSessionAdapter('gemini');
    const payload = {
      messages: [
        { id: 'g1', timestamp: '2026-04-12T00:12:13.808Z', type: 'user', content: [{ text: 'gemini prompt' }] },
        { id: 'g2', timestamp: '2026-04-12T00:12:16.701Z', type: 'gemini', content: 'gemini reply', toolCalls: [{ displayName: 'Shell' }] },
      ],
    };

    const messages = adapter.normalizeConversation(payload, 'gemini-1');
    expect(messages).toHaveLength(3);
    expect(messages[0]).toMatchObject({ id: 'g1', kind: 'user', role: 'user', provider: 'gemini' });
    expect(messages[2]).toMatchObject({ kind: 'tool', title: 'Shell' });
  });

  it('parses savant transcript json into chat messages', () => {
    const adapter = getSessionAdapter('savant');
    const payload = {
      conversation: [
        { id: 's1', timestamp: '2026-04-15T09:18:17.040230', type: 'user_message', content: 'fix the login bug' },
        { id: 's2', timestamp: '2026-04-15T09:18:18.040230', type: 'assistant_message', content: 'I will check', tool_requests: [{ tool_name: 'read_file' }] },
      ],
    };

    const messages = adapter.normalizeConversation(payload, 'savant-1');
    expect(messages).toHaveLength(3);
    expect(messages[0]).toMatchObject({ id: 's1', kind: 'user', role: 'user', provider: 'savant' });
    expect(messages[2]).toMatchObject({ kind: 'tool', title: 'read_file' });
  });

  it('treats agy and agt as provider aliases', () => {
    const agyAdapter = getSessionAdapter('agy');
    const agtAdapter = getSessionAdapter('agt');

    expect(agyAdapter.displayName).toBe('AGY');
    expect(agtAdapter.displayName).toBe('AGY');
    expect(agyAdapter.conversationPath('agy-1')).toContain('/api/agy/session/agy-1/conversation');
    expect(agtAdapter.transcriptPath({ filePath: '/Users/home/.agy/sessions/agt-1.jsonl' })).toContain('agt-1');
  });

  it('supports Hermes and shows its OpenAI-style tool calls as transcript events', () => {
    const adapter = getSessionAdapter('hermes-agent');
    const messages = adapter.normalizeConversationText(JSON.stringify({ messages: [
      { type: 'user', content: 'inspect session state' },
      { type: 'assistant', content: 'Checking.', tool_calls: [{ function: { name: 'read_file', arguments: '{"path":"state.db"}' } }] },
    ] }), 'hermes-1');
    expect(adapter.displayName).toBe('Hermes');
    expect(adapter.conversationPath('hermes-1')).toContain('/api/hermes/session/hermes-1/conversation');
    expect(messages).toContainEqual(expect.objectContaining({ kind: 'tool', title: 'read_file', detail: '{"path":"state.db"}', provider: 'hermes' }));
  });

  it('keeps the supported provider matrix explicit', () => {
    expect(['codex', 'claude', 'copilot', 'hermes', 'agy']).toEqual(
      ['codex', 'claude', 'copilot', 'hermes', 'agy'].filter((provider) => getSessionAdapter(provider).displayName !== 'Savant'),
    );
  });

  it('prefers provider transcript filenames when present', () => {
    expect(getSessionAdapter('codex').transcriptPath({ filePath: 'rollout-2025-11-20T22-42-15-abc.jsonl' })).toContain('rollout-');
    expect(getSessionAdapter('copilot').transcriptPath({ filePath: 'events.jsonl' })).toContain('events.jsonl');
    expect(getSessionAdapter('claude').transcriptPath({ filePath: 'session-abc.jsonl' })).toContain('session-abc.jsonl');
    expect(getSessionAdapter('gemini').transcriptPath({ filePath: 'session-abc.json' })).toContain('session-abc.json');
    expect(getSessionAdapter('savant').transcriptPath({ filePath: 'session_abc.json' })).toContain('session_abc.json');
  });

  it('skips codex conversation fetches and uses transcript fallback', () => {
    const adapter = getSessionAdapter('codex');
    expect(adapter.conversationPath('codex-1')).toBeNull();
    expect(adapter.transcriptPath({ filePath: 'rollout-2025-11-20T22-42-15-codex-1.jsonl' })).toContain('rollout-');
  });

  it('normalizes session details and infers providers correctly', () => {
    const rawSession = {
      id: 'sess-123',
      workspace_id: 'ws-abc',
      summary: 'Raw Session Name',
      provider: 'UNKNOWN_PROVIDER',
      model: 'Test Model',
      updated_at: new Date(Date.now() - 5 * 60000).toISOString(), // 5m ago
      file_count: 3,
    };

    const fileGroup = {
      files: [{ path: 'src/main.py' }, { path: 'src/utils.py' }]
    };

    // Should fall back to savant because of unknown provider
    const adapter = getSessionAdapter('unknown');
    const normalized = adapter.normalizeSession(rawSession, 'ws-abc', fileGroup);
    expect(normalized).not.toBeNull();
    expect(normalized!.provider).toBe('Savant');
    expect(normalized!.updated).toBe('5m ago');
    expect(normalized!.tree).toBe('src/main.py · src/utils.py'); // Savant uses dot
  });

  it('preserves the provider-reported title and agent type for the session UI', () => {
    const normalized = getSessionAdapter('codex').normalizeSession({
      id: 'sess-observed',
      provider: 'codex',
      thread_name: 'Session linkage',
      agent_type: 'codex-cli',
      model: 'gpt-5.4',
    }, 'ws-observed');

    expect(normalized).toMatchObject({
      title: 'Session linkage',
      provider: 'Codex',
      agentType: 'codex-cli',
      model: 'gpt-5.4',
    });
  });

  it('uses specific tree indicators based on inferred provider', () => {
    const rawSession = { id: 's-123', model: 'm1' };
    const files = { files: [{ path: 'a.js' }, { path: 'b.js' }] };

    const codex = getSessionAdapter('codex').normalizeSession(rawSession, 'ws', files);
    expect(codex!.tree).toBe('a.js -> b.js');

    const claude = getSessionAdapter('claude').normalizeSession(rawSession, 'ws', files);
    expect(claude!.tree).toBe('a.js / b.js');

    const gemini = getSessionAdapter('gemini').normalizeSession(rawSession, 'ws', files);
    expect(gemini!.tree).toBe('a.js • b.js');
  });

  it('normalizes updated time labels correctly', () => {
    const adapter = getSessionAdapter('savant');
    
    // Now
    const sessNow = adapter.normalizeSession({ id: 's1', updated_at: new Date().toISOString() }, 'ws');
    expect(sessNow!.updated).toBe('now');

    // Hours
    const sessHours = adapter.normalizeSession({ id: 's1', updated_at: new Date(Date.now() - 3 * 3600000).toISOString() }, 'ws');
    expect(sessHours!.updated).toBe('3h ago');

    // Days
    const sessDays = adapter.normalizeSession({ id: 's1', updated_at: new Date(Date.now() - 48 * 3600000).toISOString() }, 'ws');
    expect(sessDays!.updated).toBe('2d ago');

    // Empty/Server fallback
    const sessEmpty = adapter.normalizeSession({ id: 's1' }, 'ws');
    expect(sessEmpty!.updated).toBe('server');
  });

  it('normalizes transcript text from various JSON schemas', () => {
    const adapter = getSessionAdapter('savant');

    // JSON Array
    const arrMsg = adapter.normalizeConversationText(
      JSON.stringify([{ type: 'user', content: 'hello array' }]),
      's1'
    );
    expect(arrMsg).toHaveLength(1);
    expect(arrMsg[0].detail).toBe('hello array');

    // JSON Object with events
    const evMsg = adapter.normalizeConversationText(
      JSON.stringify({ events: [{ type: 'assistant', content: 'hello events' }] }),
      's1'
    );
    expect(evMsg).toHaveLength(1);
    expect(evMsg[0].detail).toBe('hello events');

    // JSON Object with conversation
    const convMsg = adapter.normalizeConversationText(
      JSON.stringify({ conversation: [{ type: 'system', content: 'hello conv' }] }),
      's1'
    );
    expect(convMsg).toHaveLength(1);
    expect(convMsg[0].detail).toBe('hello conv');

    // JSON Object with data
    const dataMsg = adapter.normalizeConversationText(
      JSON.stringify({ data: [{ type: 'user', content: 'hello data' }] }),
      's1'
    );
    expect(dataMsg).toHaveLength(1);
    expect(dataMsg[0].detail).toBe('hello data');

    // JSON Object with single record
    const singleMsg = adapter.normalizeConversationText(
      JSON.stringify({ type: 'user', content: 'hello single' }),
      's1'
    );
    expect(singleMsg).toHaveLength(1);
    expect(singleMsg[0].detail).toBe('hello single');

    // Empty system message in normalizeTranscriptEvents (should be skipped)
    const emptySysMsg = adapter.normalizeConversationText(
      JSON.stringify({ type: 'system', content: '' }),
      's1'
    );
    expect(emptySysMsg).toHaveLength(0);
  });

  it('handles empty and invalid timestamps/metadata', () => {
    const adapter = getSessionAdapter('savant');

    // Invalid timestamp string
    const testMsg = adapter.normalizeConversation(
      {
        conversation: [
          { id: '1', type: 'user_message', content: 'text', timestamp: 'not-a-date' },
          { id: '2', type: 'system_message', content: '' }, // empty system message should be skipped
        ]
      },
      's1'
    );
    expect(testMsg).toHaveLength(1);
    expect(testMsg[0].time).toBe('not-a-date');
  });

  it('normalizes session details with empty id or complex structures', () => {
    const adapter = getSessionAdapter('savant');
    // Empty session ID should return null
    expect(adapter.normalizeSession({}, 'ws-abc')).toBeNull();

    // Nested array content in readText
    const complexMsg = adapter.normalizeConversation(
      {
        conversation: [
          { id: 'c1', type: 'user', content: [{ text: 'part1' }, { text: 'part2' }] }
        ]
      },
      's1'
    );
    expect(complexMsg).toHaveLength(1);
    expect(complexMsg[0].detail).toBe('part1\npart2');
  });

  it('covers remaining branches in sessionAdapters helper functions', () => {
    const adapter = getSessionAdapter('savant');

    // record.entries
    const entriesMsg = adapter.normalizeConversation({ entries: [{ type: 'user', content: 'entries content' }] }, 's1');
    expect(entriesMsg).toHaveLength(1);
    expect(entriesMsg[0].detail).toBe('entries content');

    // record.data
    const dataMsg = adapter.normalizeConversation({ data: [{ type: 'user', content: 'data content' }] }, 's1');
    expect(dataMsg).toHaveLength(1);
    expect(dataMsg[0].detail).toBe('data content');

    // invalid payload types
    expect(adapter.normalizeConversation(null, 's1')).toHaveLength(0);
    expect(adapter.normalizeConversation({}, 's1')).toHaveLength(0);

    // invalid JSONL line
    const invalidJsonl = adapter.normalizeConversationText('{"type":"user", "content":"valid"}\n{invalid-json}', 's1');
    expect(invalidJsonl).toHaveLength(1);
    expect(invalidJsonl[0].detail).toBe('valid');

    // empty text
    expect(adapter.normalizeConversationText('   ', 's1')).toHaveLength(0);
  });

  it('covers pickTranscriptPath file-matching logic', () => {
    const raw = {};

    // savant match
    expect(
      getSessionAdapter('savant').transcriptPath(raw, {
        files: [{ name: 'session_123.json' }]
      })
    ).toBe('session_123.json');

    // codex match
    expect(
      getSessionAdapter('codex').transcriptPath(raw, {
        files: [{ name: 'rollout-abc.jsonl' }]
      })
    ).toBe('rollout-abc.jsonl');

    // claude match
    expect(
      getSessionAdapter('claude').transcriptPath(raw, {
        files: [{ name: 'session.jsonl' }]
      })
    ).toBe('session.jsonl');

    // gemini match
    expect(
      getSessionAdapter('gemini').transcriptPath(raw, {
        files: [{ name: 'session-xyz.json' }]
      })
    ).toBe('session-xyz.json');

    // copilot match
    expect(
      getSessionAdapter('copilot').transcriptPath(raw, {
        files: [{ name: 'events.jsonl' }]
      })
    ).toBe('events.jsonl');

    // agy match
    expect(
      getSessionAdapter('agy').transcriptPath(raw, {
        files: [{ name: 'session.db' }]
      })
    ).toBe('session.db');

    // fallback when no match but files present
    expect(
      getSessionAdapter('savant').transcriptPath(raw, {
        files: [{ name: 'random.txt' }]
      })
    ).toBe('random.txt');

    // null when no files
    expect(getSessionAdapter('savant').transcriptPath(raw, { files: [] })).toBeNull();

    // unknown provider pickTranscriptPath fallback files[0]
    expect(
      getSessionAdapter('unknown').transcriptPath(raw, {
        files: [{ name: 'unknown-file.txt' }]
      })
    ).toBe('unknown-file.txt');
  });

  it('covers other provider fallback in tree builder', () => {
    const rawSession = { id: 's-123', model: 'm1' };
    const files = { files: [{ path: 'a.js' }, { path: 'b.js' }] };

    // copilot provider tree builder uses '->' arrow join
    const copilot = getSessionAdapter('copilot').normalizeSession(rawSession, 'ws', files);
    expect(copilot!.tree).toBe('a.js -> b.js');
  });
});
