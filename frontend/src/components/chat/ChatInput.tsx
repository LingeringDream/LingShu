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
  is_active?: boolean;
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
  isActiveProvider: boolean;
}

const defaultEnhanceSettings: EnhanceSettings = {
  enabled_default: false,
  members: [],
  aggregator: null,
  per_model_timeout_ms: 30000,
  max_parallel: 4,
};

function modelOptionToConfig(option: EnhanceModelOption): EnhanceMemberConfig {
  return { provider_id: option.providerId, model: option.model };
}

function configKey(config: EnhanceMemberConfig): string {
  return `${config.provider_id}::${config.model}`;
}

function fillDefaultEnhanceSelection(
  settings: EnhanceSettings,
  options: EnhanceModelOption[],
): EnhanceSettings {
  if (options.length === 0) return settings;

  const optionKeys = new Set(options.map((option) => option.key));
  const members = settings.members.filter((member) => optionKeys.has(configKey(member)));
  const aggregator = settings.aggregator && optionKeys.has(configKey(settings.aggregator))
    ? settings.aggregator
    : null;

  if (members.length > 0 && aggregator) {
    return { ...settings, members, aggregator };
  }

  const activeOptions = options.filter((option) => option.isActiveProvider);
  const defaultMemberOptions = activeOptions.length > 0 ? activeOptions : options;
  const defaultMembers = members.length > 0 ? members : defaultMemberOptions.map(modelOptionToConfig);
  const defaultMemberKeys = new Set(defaultMembers.map(configKey));
  const defaultAggregator = options.find((option) => defaultMemberKeys.has(option.key) && option.isActiveProvider)
    ?? options.find((option) => defaultMemberKeys.has(option.key))
    ?? activeOptions[0]
    ?? options[0];

  return {
    ...settings,
    members: defaultMembers,
    aggregator: aggregator ?? modelOptionToConfig(defaultAggregator),
  };
}

export function ChatInput({ onSend, disabled }: ChatInputProps) {
  const [value, setValue] = useState('');
  const [enhanceSettings, setEnhanceSettings] = useState<EnhanceSettings | null>(null);
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
          isActiveProvider: Boolean(provider.is_active),
        })),
      );

      setEnhanceSettings(fillDefaultEnhanceSelection(settings, options));
      setEnhanceConfigLoaded(true);
    } catch (e) {
      setEnhanceSettings(defaultEnhanceSettings);
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
          title={enhanceConfigError ?? (enhanceEnabled ? '关闭增强输出' : '开启增强输出')}
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
          <StarClusterIcon
            size={18}
            title={enhanceConfigLoading
              ? '增强配置加载中'
              : enhanceEnabled ? '增强输出已开启' : '增强输出已关闭'}
          />
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
