import { useCallback, useEffect, useState, KeyboardEvent } from 'react';
import { useChatStore, type EnhanceMemberConfig, type EnhanceRequestOverride } from '../../stores/chatStore';
import { apiFetch } from '../../lib/api';
import { StarClusterIcon } from '../icons';

interface ChatInputProps {
  onSend: (message: string, enhanceOverride?: EnhanceRequestOverride) => void;
  disabled?: boolean;
}

interface EnhanceSettings {
  enabled_default: boolean;
  members: EnhanceMemberConfig[];
  aggregator: EnhanceMemberConfig | null;
  per_model_timeout_ms: number;
  max_parallel: number;
}

interface ProviderModel {
  id: string;
  name: string;
  display_name?: string;
}

interface SavedProvider {
  provider_id: string;
  display_name: string;
  models: ProviderModel[];
}

interface ProvidersResponse {
  saved: SavedProvider[];
}

interface EnhanceModelOption {
  key: string;
  providerId: string;
  model: string;
  label: string;
}

const defaultEnhanceSettings: EnhanceSettings = {
  enabled_default: false,
  members: [],
  aggregator: null,
  per_model_timeout_ms: 30000,
  max_parallel: 4,
};

export function ChatInput({ onSend, disabled }: ChatInputProps) {
  const [value, setValue] = useState('');
  const [enhanceSettings, setEnhanceSettings] = useState<EnhanceSettings | null>(null);
  const [enhanceOptions, setEnhanceOptions] = useState<EnhanceModelOption[]>([]);
  const [enhanceConfigLoaded, setEnhanceConfigLoaded] = useState(false);
  const [enhanceConfigLoading, setEnhanceConfigLoading] = useState(false);
  const [enhanceConfigError, setEnhanceConfigError] = useState<string | null>(null);
  const enhanceEnabled = useChatStore((state) => state.enhanceEnabled);
  const setEnhanceEnabled = useChatStore((state) => state.setEnhanceEnabled);
  const loadEnhanceDefault = useChatStore((state) => state.loadEnhanceDefault);

  useEffect(() => {
    void loadEnhanceDefault();
  }, [loadEnhanceDefault]);

  const loadEnhanceConfig = useCallback(async () => {
    if (enhanceConfigLoading) return;
    setEnhanceConfigLoading(true);
    setEnhanceConfigError(null);
    try {
      const [settingsResp, providersResp] = await Promise.all([
        apiFetch('/api/v1/settings/enhance'),
        apiFetch('/api/v1/settings/providers'),
      ]);

      if (settingsResp.status === 404) {
        setEnhanceSettings(defaultEnhanceSettings);
        setEnhanceOptions([]);
        setEnhanceConfigError('当前后端不支持增强配置');
        setEnhanceConfigLoaded(true);
        return;
      }
      if (!settingsResp.ok) throw new Error(`HTTP ${settingsResp.status}`);

      const settings: EnhanceSettings = await settingsResp.json();
      const providers: ProvidersResponse = providersResp.ok
        ? await providersResp.json()
        : { saved: [] };
      const options = providers.saved.flatMap((provider) =>
        provider.models.map((model) => ({
          key: `${provider.provider_id}::${model.id}`,
          providerId: provider.provider_id,
          model: model.id,
          label: `${provider.display_name} · ${model.display_name || model.name || model.id}`,
        })),
      );

      setEnhanceSettings(settings);
      setEnhanceOptions(options);
      setEnhanceConfigLoaded(true);
    } catch (e) {
      setEnhanceSettings(defaultEnhanceSettings);
      setEnhanceOptions([]);
      setEnhanceConfigError(e instanceof Error ? e.message : '加载失败');
      setEnhanceConfigLoaded(true);
    } finally {
      setEnhanceConfigLoading(false);
    }
  }, [enhanceConfigLoading]);

  useEffect(() => {
    if (enhanceEnabled && !enhanceConfigLoaded) {
      void loadEnhanceConfig();
    }
  }, [enhanceConfigLoaded, enhanceEnabled, loadEnhanceConfig]);

  const selectedMemberKeys = new Set(
    enhanceSettings?.members.map((member) => `${member.provider_id}::${member.model}`) ?? [],
  );
  const aggregatorKey = enhanceSettings?.aggregator
    ? `${enhanceSettings.aggregator.provider_id}::${enhanceSettings.aggregator.model}`
    : '';

  const optionToConfig = (key: string): EnhanceMemberConfig | null => {
    const option = enhanceOptions.find((item) => item.key === key);
    return option ? { provider_id: option.providerId, model: option.model } : null;
  };

  const toggleEnhanceMember = (option: EnhanceModelOption) => {
    setEnhanceSettings((current) => {
      const settings = current ?? defaultEnhanceSettings;
      const exists = settings.members.some((member) => `${member.provider_id}::${member.model}` === option.key);
      const members = exists
        ? settings.members.filter((member) => `${member.provider_id}::${member.model}` !== option.key)
        : [...settings.members, { provider_id: option.providerId, model: option.model }];
      return { ...settings, members };
    });
  };

  const setEnhanceAggregator = (key: string) => {
    setEnhanceSettings((current) => ({
      ...(current ?? defaultEnhanceSettings),
      aggregator: optionToConfig(key),
    }));
  };

  const selectedEnhanceOverride = (): EnhanceRequestOverride | undefined => {
    if (!enhanceEnabled || !enhanceSettings) return undefined;
    const override: EnhanceRequestOverride = {
      members: enhanceSettings.members,
      per_model_timeout_ms: enhanceSettings.per_model_timeout_ms,
      max_parallel: enhanceSettings.max_parallel,
    };
    if (enhanceSettings.aggregator) {
      override.aggregator = enhanceSettings.aggregator;
    }
    return override;
  };

  const enhanceReady = !enhanceEnabled
    || (
      enhanceConfigLoaded
      && !enhanceConfigLoading
      && !enhanceConfigError
      && Boolean(enhanceSettings?.aggregator)
      && (enhanceSettings?.members.length ?? 0) > 0
    );
  const sendDisabled = disabled || !value.trim() || !enhanceReady;

  const handleSend = () => {
    const trimmed = value.trim();
    if (trimmed && !sendDisabled) {
      onSend(trimmed, selectedEnhanceOverride());
      setValue('');
    }
  };

  const handleKeyDown = (e: KeyboardEvent) => {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  const handleToggleEnhance = () => {
    const nextEnabled = !enhanceEnabled;
    if (nextEnabled) {
      setEnhanceConfigLoaded(false);
      setEnhanceConfigError(null);
    }
    setEnhanceEnabled(nextEnabled);
  };

  return (
    <div style={{
      borderTop: '1px solid var(--border)',
      display: 'flex',
      flexDirection: 'column',
      }}>
      {enhanceEnabled && (
        <div style={enhancePanelStyle}>
          <div style={enhancePanelHeaderStyle}>
            <span style={enhancePanelTitleStyle}>
              <StarClusterIcon size={13} />
              参与模型
            </span>
            <span style={enhancePanelMetaStyle}>
              {enhanceSettings?.members.length ?? 0} 个
            </span>
          </div>

          {enhanceConfigLoading && (
            <span style={enhanceHintStyle}>加载中...</span>
          )}
          {enhanceConfigError && (
            <span style={{ ...enhanceHintStyle, color: 'var(--error)' }}>{enhanceConfigError}</span>
          )}
          {!enhanceConfigLoading && !enhanceConfigError && enhanceOptions.length === 0 && (
            <span style={enhanceHintStyle}>未配置可选模型</span>
          )}
          {!enhanceConfigLoading && !enhanceConfigError && enhanceOptions.length > 0 && (
            <>
              <div style={enhanceOptionGridStyle}>
                {enhanceOptions.map((option) => (
                  <label
                    key={option.key}
                    style={{
                      ...enhanceOptionStyle,
                      borderColor: selectedMemberKeys.has(option.key) ? 'var(--accent)' : 'var(--border)',
                      background: selectedMemberKeys.has(option.key) ? 'var(--accent-soft)' : 'var(--bg-tertiary)',
                    }}
                  >
                    <input
                      type="checkbox"
                      checked={selectedMemberKeys.has(option.key)}
                      onChange={() => toggleEnhanceMember(option)}
                    />
                    <span style={enhanceOptionLabelStyle}>{option.label}</span>
                  </label>
                ))}
              </div>
              <label style={enhanceAggregatorRowStyle}>
                <span style={enhanceSmallLabelStyle}>整合</span>
                <select
                  aria-label="增强整合模型"
                  value={aggregatorKey}
                  onChange={(e) => setEnhanceAggregator(e.target.value)}
                  style={enhanceSelectStyle}
                >
                  <option value="">未选择</option>
                  {enhanceOptions.map((option) => (
                    <option key={option.key} value={option.key}>
                      {option.label}
                    </option>
                  ))}
                </select>
              </label>
            </>
          )}
        </div>
      )}
      <div style={{
        padding: '16px 18px',
        display: 'flex',
        gap: '12px',
      }}>
        <button
          type="button"
          onClick={handleToggleEnhance}
          disabled={disabled}
          aria-pressed={enhanceEnabled}
          title={enhanceEnabled ? '关闭增强输出' : '开启增强输出'}
          style={{
            width: 42,
            height: 42,
            flex: '0 0 42px',
            display: 'grid',
            placeItems: 'center',
            background: enhanceEnabled ? 'var(--accent-soft)' : 'var(--bg-tertiary)',
            border: `1px solid ${enhanceEnabled ? 'var(--accent)' : 'var(--border)'}`,
            borderRadius: '6px',
            color: enhanceEnabled ? 'var(--accent)' : 'var(--text-secondary)',
            cursor: disabled ? 'not-allowed' : 'pointer',
            opacity: disabled ? 0.55 : 1,
          }}
        >
          <StarClusterIcon size={18} title={enhanceEnabled ? '增强输出已开启' : '增强输出已关闭'} />
        </button>
        <textarea
          value={value}
          onChange={(e) => setValue(e.target.value)}
          onKeyDown={handleKeyDown}
          disabled={disabled}
          placeholder="输入消息..."
          rows={1}
          style={{
            flex: 1,
            minWidth: 0,
            background: 'var(--bg-tertiary)',
            border: '1px solid var(--border)',
            borderRadius: '6px',
            padding: '12px 16px',
            color: 'var(--text-primary)',
            fontSize: '14px',
            resize: 'none',
            outline: 'none',
            fontFamily: 'inherit',
          }}
        />
        <button
          onClick={handleSend}
          disabled={sendDisabled}
          style={{
            background: 'var(--accent)',
            border: 'none',
            borderRadius: '6px',
            padding: '12px 24px',
            color: '#fff',
            fontSize: '14px',
            cursor: sendDisabled ? 'not-allowed' : 'pointer',
            opacity: sendDisabled ? 0.5 : 1,
          }}
        >
          发送
        </button>
      </div>
    </div>
  );
}

const enhancePanelStyle: React.CSSProperties = {
  display: 'flex',
  flexDirection: 'column',
  gap: 10,
  padding: '12px 18px 0',
};

const enhancePanelHeaderStyle: React.CSSProperties = {
  display: 'flex',
  alignItems: 'center',
  justifyContent: 'space-between',
  gap: 12,
};

const enhancePanelTitleStyle: React.CSSProperties = {
  display: 'inline-flex',
  alignItems: 'center',
  gap: 6,
  fontSize: 12,
  fontWeight: 650,
  color: 'var(--text-primary)',
};

const enhancePanelMetaStyle: React.CSSProperties = {
  fontSize: 12,
  color: 'var(--text-secondary)',
};

const enhanceHintStyle: React.CSSProperties = {
  fontSize: 12,
  color: 'var(--text-secondary)',
};

const enhanceOptionGridStyle: React.CSSProperties = {
  display: 'grid',
  gridTemplateColumns: 'repeat(auto-fit, minmax(160px, 1fr))',
  gap: 8,
};

const enhanceOptionStyle: React.CSSProperties = {
  display: 'flex',
  alignItems: 'center',
  gap: 8,
  minWidth: 0,
  minHeight: 34,
  border: '1px solid var(--border)',
  borderRadius: 6,
  padding: '6px 8px',
  color: 'var(--text-primary)',
  fontSize: 12,
};

const enhanceOptionLabelStyle: React.CSSProperties = {
  minWidth: 0,
  overflow: 'hidden',
  textOverflow: 'ellipsis',
  whiteSpace: 'nowrap',
};

const enhanceAggregatorRowStyle: React.CSSProperties = {
  display: 'grid',
  gridTemplateColumns: '42px minmax(0, 1fr)',
  alignItems: 'center',
  gap: 8,
};

const enhanceSmallLabelStyle: React.CSSProperties = {
  fontSize: 12,
  color: 'var(--text-secondary)',
};

const enhanceSelectStyle: React.CSSProperties = {
  minWidth: 0,
  width: '100%',
  background: 'var(--bg-tertiary)',
  border: '1px solid var(--border)',
  borderRadius: 6,
  padding: '8px 10px',
  color: 'var(--text-primary)',
  fontSize: 12,
  outline: 'none',
};
