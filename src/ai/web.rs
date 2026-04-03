pub fn index_html(refresh_ms: u64) -> String {
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>ai</title>
  <style>
    :root {{
      color-scheme: dark;
      --bg: #0e1116;
      --panel: #151b24;
      --line: #253142;
      --text: #d9e2f0;
      --muted: #93a4ba;
      --accent: #66b3ff;
      --warn: #e5b567;
      --danger: #ff8080;
      --ok: #7ad08a;
      font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
    }}
    * {{ box-sizing: border-box; }}
    body {{ margin: 0; background: var(--bg); color: var(--text); }}
    header {{
      padding: 16px 20px;
      border-bottom: 1px solid var(--line);
      display: flex;
      justify-content: space-between;
      align-items: center;
    }}
    main {{
      display: grid;
      grid-template-columns: 1.15fr 0.85fr;
      gap: 16px;
      padding: 16px;
    }}
    section {{
      background: var(--panel);
      border: 1px solid var(--line);
      border-radius: 14px;
      padding: 14px;
      min-height: 180px;
    }}
    h1, h2, h3 {{ margin: 0 0 10px; font-size: 14px; }}
    .stack {{ display: grid; gap: 16px; }}
    .list {{ display: grid; gap: 10px; }}
    .row {{
      display: flex;
      justify-content: space-between;
      align-items: start;
      gap: 12px;
      padding: 10px 0;
      border-top: 1px solid rgba(255,255,255,.05);
    }}
    .row:first-child {{ border-top: 0; padding-top: 0; }}
    .meta {{ color: var(--muted); font-size: 12px; }}
    .badge {{
      display: inline-block;
      border: 1px solid var(--line);
      border-radius: 999px;
      padding: 2px 8px;
      font-size: 11px;
      color: var(--muted);
    }}
    .badge.running {{ color: var(--ok); }}
    .badge.waiting {{ color: var(--warn); }}
    .badge.error {{ color: var(--danger); }}
    .controls, .approval-actions {{ display: flex; gap: 8px; flex-wrap: wrap; }}
    button {{
      border: 1px solid var(--line);
      background: transparent;
      color: var(--text);
      border-radius: 8px;
      padding: 6px 10px;
      cursor: pointer;
    }}
    button:hover {{ border-color: var(--accent); color: var(--accent); }}
    button.danger:hover {{ color: var(--danger); border-color: var(--danger); }}
    .chat-log {{
      height: 320px;
      overflow: auto;
      display: grid;
      gap: 10px;
      padding-right: 4px;
    }}
    .message {{ border-top: 1px solid rgba(255,255,255,.05); padding-top: 10px; }}
    .message:first-child {{ border-top: 0; padding-top: 0; }}
    .message .role {{ color: var(--accent); font-size: 12px; }}
    .message.assistant .role {{ color: var(--ok); }}
    .message.system .role {{ color: var(--warn); }}
    textarea, select, input {{
      width: 100%;
      background: #0c1016;
      color: var(--text);
      border: 1px solid var(--line);
      border-radius: 8px;
      padding: 8px 10px;
      font: inherit;
    }}
    textarea {{ min-height: 88px; resize: vertical; }}
    .chat-form {{ display: grid; gap: 10px; margin-top: 12px; }}
    .logs {{
      height: 240px;
      overflow: auto;
      white-space: pre-wrap;
      color: var(--muted);
      font-size: 12px;
    }}
    @media (max-width: 980px) {{
      main {{ grid-template-columns: 1fr; }}
    }}
  </style>
</head>
<body>
  <header>
    <strong>ai companion</strong>
    <span class="meta" id="connection-status">live stream</span>
  </header>
  <main>
    <div class="stack">
      <section>
        <h2>Agent Manager</h2>
        <div id="agents" class="list"></div>
      </section>
      <section>
        <h2>Status</h2>
        <div id="status" class="list"></div>
      </section>
      <section>
        <h2>Live Activity</h2>
        <div id="activity" class="list"></div>
      </section>
      <section>
        <h2>Chat</h2>
        <div id="chat-log" class="chat-log"></div>
        <form id="chat-form" class="chat-form">
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:10px;">
            <select id="provider">
              <option value="openai">OpenAI</option>
              <option value="anthropic">Anthropic</option>
              <option value="openrouter">OpenRouter</option>
            </select>
            <input id="model" placeholder="Model name">
          </div>
          <input id="system-prompt" placeholder="System prompt (optional)">
          <textarea id="message" placeholder="Ask for suggestions or create agent work"></textarea>
          <div class="controls">
            <button type="submit">Send</button>
            <button type="button" id="send-agent">Send to Agent</button>
          </div>
        </form>
      </section>
    </div>
    <div class="stack">
      <section>
        <h2>Approvals</h2>
        <div id="approvals" class="list"></div>
      </section>
      <section>
        <h2>Stream</h2>
        <div id="logs" class="logs"></div>
      </section>
    </div>
  </main>
  <script>
    const refreshMs = {refresh_ms};
    let activeSessionId = null;
    let pollHandle = null;
    let stream = null;

    async function readJson(path, options) {{
      const response = await fetch(path, options);
      if (!response.ok) {{
        throw new Error(await response.text());
      }}
      return response.json();
    }}

    async function refresh() {{
      const [agents, approvals, logs, sessions] = await Promise.all([
        readJson('/agents'),
        readJson('/approvals'),
        readJson('/logs'),
        readJson('/chat/session')
      ]);
      renderAgents(agents);
      renderStatus(agents);
      renderActivity(agents, approvals, logs);
      renderApprovals(approvals);
      renderLogs(logs);
      renderChat(sessions);
    }}

    function applySnapshot(payload) {{
      renderAgents(payload.agents || []);
      renderStatus(payload.agents || []);
      renderActivity(payload.agents || [], payload.approvals || [], payload.logs || []);
      renderApprovals(payload.approvals || []);
      renderLogs(payload.logs || []);
      renderChat(payload.chat || {{ sessions: [], active_session: null }});
    }}

    function renderAgents(agents) {{
      const root = document.getElementById('agents');
      root.innerHTML = agents.map((agent, index) => `
        <div class="row">
          <div>
            <div>${{agentLabel(agent, index)}}</div>
            <div class="meta">${{agent.command}}</div>
          </div>
          <div>
            <div class="badge ${{agent.state}}">${{agent.state}}</div>
            <div class="controls" style="margin-top:8px;">
              <button onclick="controlAgent('${{agent.id}}','start')">Start</button>
              <button onclick="controlAgent('${{agent.id}}','pause')">Pause</button>
              <button onclick="controlAgent('${{agent.id}}','resume')">Resume</button>
              <button class="danger" onclick="controlAgent('${{agent.id}}','stop')">Stop</button>
            </div>
          </div>
        </div>
      `).join('') || '<div class="meta">No agents configured.</div>';
    }}

    function renderStatus(agents) {{
      const root = document.getElementById('status');
      root.innerHTML = agents.map(agent => `
        <div class="row">
          <div>${{shortAgentName(agent)}}</div>
          <div class="badge ${{agent.state}}">${{agent.state}}</div>
        </div>
      `).join('') || '<div class="meta">No configured local CLIs.</div>';
    }}

    function renderActivity(agents, approvals, logs) {{
      const root = document.getElementById('activity');
      root.innerHTML = agents.map((agent, index) => `
        <div class="row">
          <div>
            <div>${{agentLabel(agent, index)}}</div>
            <div class="meta">${{escapeHtml(activitySummary(agent, approvals, logs))}}</div>
          </div>
        </div>
      `).join('') || '<div class="meta">No live activity yet.</div>';
    }}

    function renderApprovals(items) {{
      const root = document.getElementById('approvals');
      root.innerHTML = items.map(item => `
        <div class="row">
          <div>
            <div>${{escapeHtml(approvalAgentLabel(item))}} wants to:</div>
            <div class="meta">${{escapeHtml(item.details || item.action)}}</div>
          </div>
          <div class="approval-actions">
            <button onclick="resolveApproval('${{item.id}}','approve')">Approve</button>
            <button class="danger" onclick="resolveApproval('${{item.id}}','deny')">Deny</button>
          </div>
        </div>
      `).join('') || '<div class="meta">No approval requests.</div>';
    }}

    function renderLogs(items) {{
      document.getElementById('logs').textContent = items.map(item =>
        `[${{item.timestamp}}] ${{item.agent_id}} ${{item.level}} ${{item.message}}`
      ).join('\\n');
    }}

    function renderChat(payload) {{
      const session = payload.active_session || payload.sessions[0];
      if (!session) {{
        document.getElementById('chat-log').innerHTML = '<div class="meta">No chat session yet.</div>';
        return;
      }}
      activeSessionId = session.id;
      document.getElementById('provider').value = session.provider;
      document.getElementById('model').value = session.model;
      document.getElementById('system-prompt').value = session.system_prompt || '';
      const root = document.getElementById('chat-log');
      root.innerHTML = session.messages.map(message => `
        <div class="message ${{message.role}}">
          <div class="role">${{message.role}}</div>
          <div>${{escapeHtml(message.content)}}</div>
        </div>
      `).join('') || '<div class="meta">Start the conversation.</div>';
      root.scrollTop = root.scrollHeight;
    }}

    async function controlAgent(id, action) {{
      await fetch(`/agents/${{encodeURIComponent(id)}}/${{action}}`, {{ method: 'POST' }});
      refresh();
    }}

    async function resolveApproval(id, action) {{
      await readJson(`/${{action}}`, {{
        method: 'POST',
        headers: {{ 'Content-Type': 'application/json' }},
        body: JSON.stringify({{ id }})
      }});
      refresh();
    }}

    document.getElementById('chat-form').addEventListener('submit', async (event) => {{
      event.preventDefault();
      const content = document.getElementById('message').value.trim();
      if (!content) return;
      const session = await readJson('/chat/session', {{
        method: 'POST',
        headers: {{ 'Content-Type': 'application/json' }},
        body: JSON.stringify({{
          session_id: activeSessionId,
          provider: document.getElementById('provider').value,
          model: document.getElementById('model').value.trim() || undefined,
          system_prompt: document.getElementById('system-prompt').value.trim() || undefined
        }})
      }});
      activeSessionId = session.id;
      await readJson('/chat/message', {{
        method: 'POST',
        headers: {{ 'Content-Type': 'application/json' }},
        body: JSON.stringify({{ session_id: activeSessionId, content }})
      }});
      document.getElementById('message').value = '';
      refresh();
    }});

    document.getElementById('send-agent').addEventListener('click', async () => {{
      if (!activeSessionId) return;
      await readJson('/chat/send-to-agent', {{
        method: 'POST',
        headers: {{ 'Content-Type': 'application/json' }},
        body: JSON.stringify({{ session_id: activeSessionId }})
      }});
      refresh();
    }});

    function escapeHtml(value) {{
      return String(value ?? '')
        .replaceAll('&', '&amp;')
        .replaceAll('<', '&lt;')
        .replaceAll('>', '&gt;');
    }}

    function shortAgentName(agent) {{
      if (!agent || !agent.display_name) return agent?.id || 'Agent';
      if (agent.display_name === 'Codex CLI') return 'Codex';
      if (agent.display_name === 'Claude Code') return 'Claude Code';
      if (agent.display_name === 'Gemini CLI') return 'Gemini CLI';
      if (agent.display_name === 'Generic Agent CLI') return 'Agent';
      return agent.display_name;
    }}

    function agentLabel(agent, index = 0) {{
      return `${{shortAgentName(agent)}} (${{index + 1}})`;
    }}

    function approvalAgentLabel(item) {{
      return item.agent_id || 'Agent';
    }}

    function activitySummary(agent, approvals, logs) {{
      const pending = approvals.find(item => item.agent_id === agent.id && item.state === 'pending');
      if (agent.state === 'waiting' && pending) {{
        return `Waiting approval: ${{pending.action}}`;
      }}
      if (agent.current_task) {{
        const label = agent.current_task.state === 'running'
          ? 'Running'
          : agent.current_task.state === 'pending'
            ? 'Queued'
            : agent.current_task.state === 'failed'
              ? 'Failed'
              : 'Finished';
        return `${{label}}: ${{agent.current_task.description}}`;
      }}
      const lastLog = [...logs].reverse().find(item => item.agent_id === agent.id);
      if (lastLog) {{
        if (lastLog.level === 'task') return `Running: ${{lastLog.message}}`;
        if (lastLog.level === 'error') return `Error: ${{lastLog.message}}`;
        const lower = lastLog.message.toLowerCase();
        if (lower.includes('edit')) return `Editing: ${{lastLog.message}}`;
        if (lower.includes('cargo ') || lower.includes('npm ') || lower.includes('python ')) {{
          return `Running: ${{lastLog.message}}`;
        }}
        return lastLog.message;
      }}
      if (agent.state === 'running') return 'Reasoning...';
      if (agent.state === 'idle') return 'Idle';
      if (agent.state === 'paused') return 'Paused';
      if (agent.state === 'error') return agent.last_error || 'Error';
      return 'Waiting';
    }}

    function setConnectionStatus(value) {{
      document.getElementById('connection-status').textContent = value;
    }}

    function startPollingFallback() {{
      if (pollHandle) return;
      setConnectionStatus(`polling every ${{refreshMs}} ms`);
      refresh().catch(() => null);
      pollHandle = setInterval(() => {{
        refresh().catch(() => null);
      }}, refreshMs);
    }}

    function stopPollingFallback() {{
      if (!pollHandle) return;
      clearInterval(pollHandle);
      pollHandle = null;
    }}

    function startLiveStream() {{
      if (!window.EventSource) {{
        startPollingFallback();
        return;
      }}
      const source = new EventSource('/stream');
      stream = source;
      source.addEventListener('snapshot', (event) => {{
        stopPollingFallback();
        setConnectionStatus('live stream');
        applySnapshot(JSON.parse(event.data));
      }});
      source.onerror = () => {{
        if (stream === source) {{
          setConnectionStatus('stream unavailable, using polling');
          source.close();
          startPollingFallback();
        }}
      }};
    }}

    refresh().catch(() => null);
    startLiveStream();
  </script>
</body>
</html>"#
    )
}
