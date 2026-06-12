/* global CustomEvent, StorageEvent */
import { isTauri } from '../../lib/tauri';

export type AvatarMood = 'idle' | 'thinking' | 'speaking' | 'reminder';
export type AvatarSize = 'small' | 'medium' | 'large';
export type PetRuntimeMood = 'idle' | 'thinking' | 'speaking' | 'happy' | 'sleepy';

export interface AvatarControlSettings {
  visible: boolean;
  mood: AvatarMood;
  size: AvatarSize;
  bubbleText: string;
}

export const AVATAR_CONTROL_EVENT = 'lingshu:avatar-controls';
export const AVATAR_CONTROL_STORAGE_KEY = 'lingshu_avatar_controls';

export const DEFAULT_AVATAR_CONTROL_SETTINGS: AvatarControlSettings = {
  visible: true,
  mood: 'idle',
  size: 'medium',
  bubbleText: '我在这里，需要时叫我。',
};

const AVATAR_MOODS: AvatarMood[] = ['idle', 'thinking', 'speaking', 'reminder'];
const AVATAR_SIZES: AvatarSize[] = ['small', 'medium', 'large'];

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null;
}

function normalizeAvatarControlSettings(value: unknown): AvatarControlSettings {
  if (!isRecord(value)) return DEFAULT_AVATAR_CONTROL_SETTINGS;

  const mood = typeof value.mood === 'string' && AVATAR_MOODS.includes(value.mood as AvatarMood)
    ? value.mood as AvatarMood
    : DEFAULT_AVATAR_CONTROL_SETTINGS.mood;
  const size = typeof value.size === 'string' && AVATAR_SIZES.includes(value.size as AvatarSize)
    ? value.size as AvatarSize
    : DEFAULT_AVATAR_CONTROL_SETTINGS.size;
  const bubbleText = typeof value.bubbleText === 'string'
    ? value.bubbleText.slice(0, 80)
    : DEFAULT_AVATAR_CONTROL_SETTINGS.bubbleText;

  return {
    visible: typeof value.visible === 'boolean'
      ? value.visible
      : DEFAULT_AVATAR_CONTROL_SETTINGS.visible,
    mood,
    size,
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

export function avatarSizeToScale(size: AvatarSize): number {
  switch (size) {
    case 'small':
      return 0.86;
    case 'large':
      return 1.12;
    case 'medium':
    default:
      return 1;
  }
}
