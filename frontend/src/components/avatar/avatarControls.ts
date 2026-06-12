/* global CustomEvent, StorageEvent */
import { isTauri } from '../../lib/tauri';

export type AvatarMood = 'idle' | 'thinking' | 'speaking' | 'reminder';
export type PetRuntimeMood = 'idle' | 'thinking' | 'speaking' | 'happy' | 'sleepy';

export interface AvatarControlSettings {
  visible: boolean;
  mood: AvatarMood;
  sizeScale: number;
  bubbleText: string;
}

export const AVATAR_CONTROL_EVENT = 'lingshu:avatar-controls';
export const AVATAR_CONTROL_STORAGE_KEY = 'lingshu_avatar_controls';

export const DEFAULT_AVATAR_CONTROL_SETTINGS: AvatarControlSettings = {
  visible: true,
  mood: 'idle',
  sizeScale: 1,
  bubbleText: '我在这里，需要时叫我。',
};

const AVATAR_MOODS: AvatarMood[] = ['idle', 'thinking', 'speaking', 'reminder'];
const MIN_SIZE_SCALE = 0.75;
const MAX_SIZE_SCALE = 1.25;
const LEGACY_SIZE_SCALE: Record<string, number> = {
  small: 0.86,
  medium: 1,
  large: 1.12,
};

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

function clampSizeScale(value: number): number {
  if (!Number.isFinite(value)) return DEFAULT_AVATAR_CONTROL_SETTINGS.sizeScale;
  return Math.min(MAX_SIZE_SCALE, Math.max(MIN_SIZE_SCALE, value));
}

function readSizeScale(value: Record<string, unknown>): number {
  if (typeof value.sizeScale === 'number') {
    return clampSizeScale(value.sizeScale);
  }
  if (typeof value.size === 'string' && value.size in LEGACY_SIZE_SCALE) {
    return LEGACY_SIZE_SCALE[value.size];
  }
  return DEFAULT_AVATAR_CONTROL_SETTINGS.sizeScale;
}

export function normalizeAvatarControlSettings(value: unknown): AvatarControlSettings {
  if (!isRecord(value)) return DEFAULT_AVATAR_CONTROL_SETTINGS;

  const mood = typeof value.mood === 'string' && AVATAR_MOODS.includes(value.mood as AvatarMood)
    ? value.mood as AvatarMood
    : DEFAULT_AVATAR_CONTROL_SETTINGS.mood;
  const bubbleText = typeof value.bubbleText === 'string'
    ? value.bubbleText.slice(0, 80)
    : DEFAULT_AVATAR_CONTROL_SETTINGS.bubbleText;

  return {
    visible: typeof value.visible === 'boolean'
      ? value.visible
      : DEFAULT_AVATAR_CONTROL_SETTINGS.visible,
    mood,
    sizeScale: readSizeScale(value),
    bubbleText,
  };
}

export function loadAvatarControlSettings(): AvatarControlSettings {
  try {
    const stored = window.localStorage.getItem(AVATAR_CONTROL_STORAGE_KEY);
    return stored
      ? normalizeAvatarControlSettings(JSON.parse(stored))
      : DEFAULT_AVATAR_CONTROL_SETTINGS;
  } catch {
    return DEFAULT_AVATAR_CONTROL_SETTINGS;
  }
}

export function saveAvatarControlSettings(settings: AvatarControlSettings): AvatarControlSettings {
  const normalized = normalizeAvatarControlSettings(settings);
  try {
    window.localStorage.setItem(AVATAR_CONTROL_STORAGE_KEY, JSON.stringify(normalized));
  } catch {
    // Storage is best-effort; Tauri/browser events below still keep live windows in sync.
  }
  return normalized;
}

export async function publishAvatarControlSettings(settings: AvatarControlSettings): Promise<void> {
  const normalized = saveAvatarControlSettings(settings);
  window.dispatchEvent(new CustomEvent(AVATAR_CONTROL_EVENT, { detail: normalized }));

  if (!isTauri()) return;
  try {
    const { emitTo } = await import('@tauri-apps/api/event');
    await emitTo('pet', AVATAR_CONTROL_EVENT, normalized);
  } catch (error) {
    console.error('[avatar] failed to emit control settings to pet window:', error);
  }
}

export function subscribeToAvatarControlSettings(
  handler: (settings: AvatarControlSettings) => void,
): () => void {
  let disposed = false;
  let unlistenTauri: (() => void) | null = null;

  const handleCustomEvent = (event: Event) => {
    handler(normalizeAvatarControlSettings((event as CustomEvent).detail));
  };

  const handleStorageEvent = (event: StorageEvent) => {
    if (event.key !== AVATAR_CONTROL_STORAGE_KEY) return;
    handler(loadAvatarControlSettings());
  };

  window.addEventListener(AVATAR_CONTROL_EVENT, handleCustomEvent);
  window.addEventListener('storage', handleStorageEvent);

  if (isTauri()) {
    import('@tauri-apps/api/event')
      .then(({ listen }) => listen<AvatarControlSettings>(AVATAR_CONTROL_EVENT, (event) => {
        handler(normalizeAvatarControlSettings(event.payload));
      }))
      .then((unlisten) => {
        if (disposed) {
          unlisten();
        } else {
          unlistenTauri = unlisten;
        }
      })
      .catch((error) => {
        console.error('[avatar] failed to listen for control settings:', error);
      });
  }

  return () => {
    disposed = true;
    window.removeEventListener(AVATAR_CONTROL_EVENT, handleCustomEvent);
    window.removeEventListener('storage', handleStorageEvent);
    if (unlistenTauri) unlistenTauri();
  };
}

export function avatarMoodToPetMood(mood: AvatarMood): PetRuntimeMood {
  return mood === 'reminder' ? 'happy' : mood;
}
