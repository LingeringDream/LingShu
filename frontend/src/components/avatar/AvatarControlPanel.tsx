import type { AvatarControlSettings, AvatarMood } from './avatarControls';

export type { AvatarControlSettings, AvatarMood } from './avatarControls';

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

        <div className="slider-control">
          <label htmlFor="avatar-size">宠物大小</label>
          <input
            id="avatar-size"
            type="range"
            min={0.75}
            max={1.25}
            step={0.01}
            value={settings.sizeScale}
            onChange={(event) => patch({ sizeScale: Number(event.target.value) })}
          />
          <span>{Math.round(settings.sizeScale * 100)}%</span>
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
