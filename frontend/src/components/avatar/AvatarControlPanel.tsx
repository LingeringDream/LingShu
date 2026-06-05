export type AvatarMood = 'idle' | 'thinking' | 'speaking' | 'reminder';
export type AvatarSize = 'small' | 'medium' | 'large';

export interface AvatarControlSettings {
  visible: boolean;
  mood: AvatarMood;
  size: AvatarSize;
  bubbleText: string;
}

interface AvatarControlPanelProps {
  settings: AvatarControlSettings;
  onChange: (settings: AvatarControlSettings) => void;
}

const MOOD_OPTIONS: { value: AvatarMood; label: string }[] = [
  { value: 'idle', label: '待机' },
  { value: 'thinking', label: '思考' },
  { value: 'speaking', label: '说话' },
  { value: 'reminder', label: '提醒' },
];

const SIZE_OPTIONS: { value: AvatarSize; label: string }[] = [
  { value: 'small', label: '小' },
  { value: 'medium', label: '中' },
  { value: 'large', label: '大' },
];

export function AvatarControlPanel({ settings, onChange }: AvatarControlPanelProps) {
  const patch = (partial: Partial<AvatarControlSettings>) => {
    onChange({ ...settings, ...partial });
  };

  return (
    <div>
      <div className="panel-header">
        <span>虚拟形象</span>
        <label className="switch-row">
          <input
            type="checkbox"
            checked={settings.visible}
            onChange={(event) => patch({ visible: event.target.checked })}
          />
          <span>{settings.visible ? '显示' : '隐藏'}</span>
        </label>
      </div>

      <div className="panel-body">
        <div className="segmented-control">
          {MOOD_OPTIONS.map((option) => (
            <button
              key={option.value}
              type="button"
              className={settings.mood === option.value ? 'segment-active' : ''}
              onClick={() => patch({ mood: option.value })}
            >
              {option.label}
            </button>
          ))}
        </div>

        <div className="segmented-control">
          {SIZE_OPTIONS.map((option) => (
            <button
              key={option.value}
              type="button"
              className={settings.size === option.value ? 'segment-active' : ''}
              onClick={() => patch({ size: option.value })}
            >
              {option.label}
            </button>
          ))}
        </div>

        <textarea
          value={settings.bubbleText}
          onChange={(event) => patch({ bubbleText: event.target.value })}
          rows={2}
          maxLength={80}
          placeholder="气泡文案"
          className="panel-textarea"
        />
      </div>
    </div>
  );
}
