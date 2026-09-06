import { useState } from 'react';
import { apiFetch } from '../../lib/api';

type ToolId = string;
type Transport = 'stdio' | 'http';

interface ToolMeta {
  id: ToolId;
  name: string;
  desc: string;
  note: string;
  category?: string;
}

const TOOLS: ToolMeta[] = [
  // 桌面端
  {
    id: 'claude',
    name: 'Claude Desktop',
    desc: 'Anthropic 官方桌面端',
    note: '写入 claude_desktop_config.json',
    category: '桌面端',
  },
  {
    id: 'chatgpt',
    name: 'ChatGPT Desktop',
    desc: 'OpenAI 桌面应用（Beta 开发者模式）',
    note: '写入 ChatGPT/mcp.json（仅远程 HTTP MCP）',
    category: '桌面端',
  },
  {
    id: 'workbuddy',
    name: 'WorkBuddy（腾讯）',
    desc: '腾讯 AI 编程助手 / CodeBuddy',
    note: '写入 ~/.workbuddy/mcp.json',
    category: '桌面端',
  },
  {
    id: 'cherry-studio',
    name: 'Cherry Studio',
    desc: '开源多模型桌面客户端',
    note: '请复制配置片段后手动粘贴到 MCP 设置',
    category: '桌面端',
  },
  {
    id: 'opencat',
    name: 'OpenCat',
    desc: '跨平台 AI 客户端',
    note: '请复制配置片段后手动粘贴到 MCP 设置',
    category: '桌面端',
  },

  // 编辑器 / IDE
  {
    id: 'cursor',
    name: 'Cursor',
    desc: 'AI 代码编辑器',
    note: '写入 ~/.cursor/mcp.json',
    category: '编辑器 / IDE',
  },
  {
    id: 'windsurf',
    name: 'Windsurf',
    desc: 'Codeium 出品的 AI IDE',
    note: '写入 ~/.codeium/windsurf/mcp_config.json',
    category: '编辑器 / IDE',
  },
  {
    id: 'trae',
    name: 'Trae',
    desc: '字节跳动的 AI IDE',
    note: '写入 ~/.trae/mcp.json',
    category: '编辑器 / IDE',
  },
  {
    id: 'vscode',
    name: 'VS Code',
    desc: '微软官方编辑器',
    note: '写入 settings.json 的 mcp.servers',
    category: '编辑器 / IDE',
  },

  // 编码 Agent
  {
    id: 'cline',
    name: 'Cline (VS Code)',
    desc: 'VS Code 中的自主编码 Agent',
    note: '写入 settings.json 的 cline.mcpServers',
    category: '编码 Agent',
  },
  {
    id: 'roo',
    name: 'Roo Code',
    desc: 'Cline 分支，VS Code Agent',
    note: '写入 settings.json 的 roo.mcpServers',
    category: '编码 Agent',
  },
  {
    id: 'continue',
    name: 'Continue',
    desc: 'VS Code 开源编码助手',
    note: '写入 ~/.continue/config.json',
    category: '编码 Agent',
  },

  // 更多客户端
  {
    id: 'nextchat',
    name: 'NextChat',
    desc: '开源 ChatGPT 客户端',
    note: '请复制配置片段后手动粘贴到 MCP 插件设置',
    category: '更多客户端',
  },
  {
    id: 'lobechat',
    name: 'LobeChat',
    desc: '开源聊天客户端',
    note: '请复制配置片段后手动粘贴到 MCP 设置',
    category: '更多客户端',
  },
  {
    id: 'anythingllm',
    name: 'AnythingLLM',
    desc: '本地知识库与聊天',
    note: '请复制配置片段后手动粘贴到 Agent 技能设置',
    category: '更多客户端',
  },
  {
    id: 'librechat',
    name: 'LibreChat',
    desc: '开源多模型聊天平台',
    note: '请复制配置片段后手动粘贴到 MCP 插件设置',
    category: '更多客户端',
  },
  {
    id: 'dify',
    name: 'Dify',
    desc: 'AI 工作流与 Agent 平台',
    note: '请复制配置片段后手动粘贴到 MCP 工具配置',
    category: '更多客户端',
  },
  {
    id: 'manus',
    name: 'Manus',
    desc: '云端 AI 代理（无法直连本机）',
    note: '在 Manus 设置 → Connectors → Add Custom MCP 粘贴远程 URL',
    category: '更多客户端',
  },
  {
    id: 'coze',
    name: 'Coze（扣子）',
    desc: '字节跳动 AI Bot 平台',
    note: '请复制配置片段后手动粘贴到 Coze 工作流的 MCP 节点',
    category: '更多客户端',
  },
];

const CATEGORIES = Array.from(new Set(TOOLS.map((t) => t.category ?? '其他')));

interface ImportResult {
  ok: boolean;
  message: string;
  written: boolean;
  snippet?: string;
}

interface CustomForm {
  name: string;
  configPath: string;
  parentKey: string;
}

const cardBase: React.CSSProperties = {
  border: '1px solid var(--border, #e5e7eb)',
  borderRadius: 10,
  padding: '14px 16px',
  background: 'var(--card-bg, #ffffff)',
  display: 'flex',
  flexDirection: 'column',
  gap: 6,
  minHeight: 168,
  transition: 'border-color 0.15s ease, box-shadow 0.15s ease, transform 0.15s ease',
};

const cardHover: React.CSSProperties = {
  borderColor: 'var(--accent, #6c5ce7)',
  boxShadow: '0 4px 12px rgba(108, 92, 231, 0.12)',
  transform: 'translateY(-1px)',
};

const primaryBtn: React.CSSProperties = {
  padding: '6px 14px',
  border: '1px solid var(--accent, #6c5ce7)',
  borderRadius: 6,
  background: 'var(--accent, #6c5ce7)',
  color: '#ffffff',
  cursor: 'pointer',
  fontSize: 13,
  fontWeight: 500,
  transition: 'opacity 0.15s ease',
};

const ghostBtn: React.CSSProperties = {
  padding: '6px 12px',
  border: '1px solid var(--border, #e5e7eb)',
  borderRadius: 6,
  background: 'transparent',
  color: '#666',
  cursor: 'pointer',
  fontSize: 12,
};

const inputBase: React.CSSProperties = {
  padding: '7px 10px',
  fontSize: 13,
  border: '1px solid var(--border, #e5e7eb)',
  borderRadius: 6,
  outline: 'none',
  transition: 'border-color 0.15s ease',
};

export function AiToolsPanel() {
  const [transport, setTransport] = useState<Transport>('stdio');
  const [results, setResults] = useState<Record<string, ImportResult>>({});
  const [busy, setBusy] = useState<ToolId | null>(null);
  const [hoverId, setHoverId] = useState<ToolId | null>(null);

  const [custom, setCustom] = useState<CustomForm>({
    name: '',
    configPath: '',
    parentKey: '',
  });
  const [showCustom, setShowCustom] = useState(false);

  async function importTool(tool: ToolId, overrides?: Partial<CustomForm>) {
    setBusy(tool);
    try {
      const body: Record<string, unknown> = { tool, transport };
      if (overrides?.configPath) {
        body.config_path = overrides.configPath;
      }
      if (overrides?.parentKey) {
        body.parent_key = overrides.parentKey;
      }

      const res = await apiFetch('/api/v1/mcp/import', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(body),
      });
      const data = await res.json();
      if (res.ok) {
        setResults((prev) => ({
          ...prev,
          [tool]: {
            ok: true,
            written: Boolean(data.written),
            message: data.written
              ? `已写入：${data.path}`
              : '无法自动写入配置文件，请手动复制下方片段到对应位置。',
            snippet: data.snippet,
          },
        }));
      } else {
        setResults((prev) => ({
          ...prev,
          [tool]: {
            ok: false,
            written: false,
            message: `失败：${data?.error?.message ?? `HTTP ${res.status}`}`,
          },
        }));
      }
    } catch (err) {
      setResults((prev) => ({
        ...prev,
        [tool]: { ok: false, written: false, message: `失败：${String(err)}` },
      }));
    } finally {
      setBusy(null);
    }
  }

  async function importCustom() {
    const name = custom.name.trim() || '自定义工具';
    const configPath = custom.configPath.trim();
    if (!configPath) {
      setResults((prev) => ({
        ...prev,
        custom: {
          ok: false,
          written: false,
          message: '请填写配置文件路径',
        },
      }));
      return;
    }
    await importTool(name, {
      configPath,
      parentKey: custom.parentKey.trim(),
    });
  }

  async function copySnippet(snippet: string) {
    try {
      await window.navigator.clipboard.writeText(snippet);
    } catch {
      // 浏览器拒绝时忽略。
    }
  }

  function renderResultBlock(toolId: string) {
    const result = results[toolId];
    if (!result) return null;
    const tone = !result.ok
      ? { bg: 'rgba(192, 57, 43, 0.08)', fg: '#c0392b' }
      : result.written
        ? { bg: 'rgba(26, 127, 55, 0.08)', fg: '#1a7f37' }
        : { bg: 'rgba(183, 121, 31, 0.08)', fg: '#b7791f' };
    return (
      <div
        style={{
          fontSize: 12,
          marginTop: 4,
          padding: '6px 8px',
          borderRadius: 4,
          background: tone.bg,
          color: tone.fg,
          whiteSpace: 'pre-wrap',
          wordBreak: 'break-word',
        }}
      >
        {result.message}
        {result.ok && !result.written && result.snippet && (
          <button
            type="button"
            onClick={() => void copySnippet(result.snippet as string)}
            style={{
              display: 'inline-block',
              marginLeft: 6,
              fontSize: 12,
              textDecoration: 'underline',
              background: 'none',
              border: 'none',
              cursor: 'pointer',
              color: 'var(--accent, #6c5ce7)',
              padding: 0,
            }}
          >
            复制配置片段
          </button>
        )}
      </div>
    );
  }

  function renderCard(t: ToolMeta) {
    const isHover = hoverId === t.id;
    const disabled = busy === t.id;
    return (
      <div
        key={t.id}
        style={{ ...cardBase, ...(isHover ? cardHover : {}) }}
        onMouseEnter={() => setHoverId(t.id)}
        onMouseLeave={() => setHoverId(null)}
      >
        <div style={{ fontSize: 15, fontWeight: 600, color: '#1f2937' }}>{t.name}</div>
        <div
          style={{
            fontSize: 12,
            color: '#6b7280',
            lineHeight: 1.5,
            minHeight: 36,
          }}
        >
          {t.desc}
        </div>
        <div
          style={{
            fontSize: 11,
            color: '#9ca3af',
            lineHeight: 1.4,
            flexGrow: 1,
            display: 'flex',
            alignItems: 'flex-end',
          }}
        >
          {t.note}
        </div>

        <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginTop: 8 }}>
          <button
            type="button"
            disabled={disabled}
            onClick={() => importTool(t.id)}
            style={{
              ...primaryBtn,
              opacity: disabled ? 0.6 : 1,
              cursor: disabled ? 'default' : 'pointer',
            }}
          >
            {disabled ? '导入中…' : '导入配置'}
          </button>
        </div>

        {renderResultBlock(t.id)}
      </div>
    );
  }

  function renderCustomCard() {
    const disabled = busy === custom.name;
    const isHover = hoverId === 'custom';

    return (
      <div
        style={{
          ...cardBase,
          borderStyle: 'dashed',
          ...(isHover ? cardHover : {}),
        }}
        onMouseEnter={() => setHoverId('custom')}
        onMouseLeave={() => setHoverId(null)}
      >
        <div style={{ fontSize: 15, fontWeight: 600, color: '#1f2937' }}>自定义导入</div>
        <div style={{ fontSize: 12, color: '#6b7280', lineHeight: 1.5, minHeight: 36 }}>
          其他支持 MCP 协议的客户端；手动填写配置文件路径与父级 JSON 键
        </div>
        <div
          style={{
            fontSize: 11,
            color: '#9ca3af',
            lineHeight: 1.4,
            flexGrow: 1,
            display: 'flex',
            alignItems: 'flex-end',
          }}
        >
          父级键不填表示顶层 mcpServers
        </div>

        {!showCustom ? (
          <button
            type="button"
            onClick={() => setShowCustom(true)}
            style={{
              ...primaryBtn,
              background: 'transparent',
              color: 'var(--accent, #6c5ce7)',
              alignSelf: 'flex-start',
              marginTop: 8,
            }}
          >
            添加工具
          </button>
        ) : (
          <div
            style={{
              marginTop: 10,
              display: 'flex',
              flexDirection: 'column',
              gap: 8,
            }}
          >
            <input
              type="text"
              placeholder="工具名称（如 ChatWise）"
              value={custom.name}
              onChange={(e) => setCustom((c) => ({ ...c, name: e.target.value }))}
              style={inputBase}
            />
            <input
              type="text"
              placeholder="配置文件绝对路径"
              value={custom.configPath}
              onChange={(e) => setCustom((c) => ({ ...c, configPath: e.target.value }))}
              style={inputBase}
            />
            <input
              type="text"
              placeholder="父级 JSON 键，如 mcp.servers（可选）"
              value={custom.parentKey}
              onChange={(e) => setCustom((c) => ({ ...c, parentKey: e.target.value }))}
              style={inputBase}
            />
            <div style={{ display: 'flex', gap: 8 }}>
              <button
                type="button"
                disabled={disabled}
                onClick={importCustom}
                style={{
                  ...primaryBtn,
                  opacity: disabled ? 0.6 : 1,
                  cursor: disabled ? 'default' : 'pointer',
                }}
              >
                {disabled ? '导入中…' : '导入配置'}
              </button>
              <button
                type="button"
                onClick={() => {
                  setShowCustom(false);
                  setCustom({ name: '', configPath: '', parentKey: '' });
                }}
                style={ghostBtn}
              >
                取消
              </button>
            </div>

            {renderResultBlock('custom')}
          </div>
        )}
      </div>
    );
  }

  return (
    <section className="dashboard-card page-tool-card">
      <div className="card-title-row">
        <h3 style={{ margin: 0 }}>接入 AI 工具</h3>
        <span style={{ fontSize: 12, color: '#888' }}>快捷键 Ctrl / Cmd + Shift + M</span>
      </div>

      <p
        style={{
          fontSize: 13,
          color: '#666',
          margin: '8px 0 14px',
          lineHeight: 1.6,
          maxWidth: 820,
        }}
      >
        把灵枢记忆网关作为标准 MCP 服务接入任意支持 Model Context Protocol 的 AI 工具。
        一键把配置写入对应软件的配置文件，让 Claude / ChatGPT / Cursor / Cline / VS Code / Windsurf /
        Trae / Continue / Roo Code 等工具直接读写灵枢长期记忆。
      </p>

      <div
        style={{
          display: 'flex',
          gap: 18,
          alignItems: 'center',
          flexWrap: 'wrap',
          marginBottom: 16,
          padding: '8px 12px',
          background: 'var(--subtle-bg, #f9fafb)',
          borderRadius: 6,
          fontSize: 13,
        }}
      >
        <span style={{ fontWeight: 500, color: '#374151' }}>传输方式：</span>
        <label style={{ cursor: 'pointer', display: 'inline-flex', alignItems: 'center', gap: 6 }}>
          <input
            type="radio"
            name="transport"
            checked={transport === 'stdio'}
            onChange={() => setTransport('stdio')}
          />
          stdio（推荐，本地最通用）
        </label>
        <label style={{ cursor: 'pointer', display: 'inline-flex', alignItems: 'center', gap: 6 }}>
          <input
            type="radio"
            name="transport"
            checked={transport === 'http'}
            onChange={() => setTransport('http')}
          />
          HTTP + SSE（远程 / 局域网）
        </label>
      </div>

      <div
        style={{
          display: 'flex',
          flexDirection: 'column',
          gap: 16,
        }}
      >
        {CATEGORIES.map((cat) => (
          <div key={cat}>
            <div
              style={{
                fontSize: 13,
                fontWeight: 600,
                color: '#4b5563',
                marginBottom: 8,
                paddingLeft: 8,
                borderLeft: '3px solid var(--accent, #6c5ce7)',
                lineHeight: 1.2,
              }}
            >
              {cat}
            </div>
            <div
              style={{
                display: 'grid',
                gridTemplateColumns: 'repeat(auto-fill, minmax(260px, 1fr))',
                gap: 12,
              }}
            >
              {TOOLS.filter((t) => (t.category ?? '其他') === cat).map((t) => renderCard(t))}
            </div>
          </div>
        ))}

        <div>
          <div
            style={{
              fontSize: 13,
              fontWeight: 600,
              color: '#4b5563',
              marginBottom: 8,
              paddingLeft: 8,
              borderLeft: '3px dashed var(--accent, #6c5ce7)',
              lineHeight: 1.2,
            }}
          >
            自定义
          </div>
          <div
            style={{
              display: 'grid',
              gridTemplateColumns: 'repeat(auto-fill, minmax(260px, 1fr))',
              gap: 12,
            }}
          >
            {renderCustomCard()}
          </div>
        </div>
      </div>
    </section>
  );
}
