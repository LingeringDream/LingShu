import { useEffect, useMemo, useRef, useState, type CSSProperties } from 'react';

import {
  defaultModifiers,
  getMoodPresentation,
  type Mood,
  type PersonalityModifiers,
} from './petPresentation';

interface TauriOriginalPetProps {
  visible: boolean;
  sizeScale: number;
  mood: Mood;
  reactionKey: number;
  pressed: boolean;
  modifiers?: PersonalityModifiers;
}

interface PetPoint {
  cx: number;
  cy: number;
}

interface PetSparkFrame extends PetPoint {
  r: number;
  opacity: number;
}

export interface TauriPetFrameState {
  floatY: number;
  glowRadius: number;
  glowOpacity: number;
  shellRadius: number;
  shellStrokeRadius: number;
  ringRadius: number;
  coreOuterRadius: number;
  coreInnerRadius: number;
  eyeHaloRadius: number;
  starScale: number;
  orbitBackRotation: number;
  orbitFrontRotation: number;
  innerDotA: PetPoint;
  innerDotB: PetPoint;
  wakeOpacity: number;
  bodyScaleX: number;
  bodyScaleY: number;
  sparks: PetSparkFrame[];
}

const DEFAULT_TAURI_MODIFIERS = defaultModifiers();
const BASE_ORBIT_SECONDS = 2.52;
const SPARK_VECTORS = [
  { x: 24, y: -28, delay: 0, r: 3 },
  { x: -30, y: -20, delay: 30, r: 2.5 },
  { x: 33, y: 10, delay: 70, r: 3.2 },
  { x: -25, y: 22, delay: 90, r: 2.4 },
  { x: 6, y: -36, delay: 120, r: 2.8 },
  { x: -5, y: 34, delay: 150, r: 2.2 },
];

function colorToHex(color: number): string {
  return `#${color.toString(16).padStart(6, '0')}`;
}

function orbitDurationSeconds(mood: Mood, modifiers: PersonalityModifiers): string {
  const presentation = getMoodPresentation(mood);
  const speed = Math.max(0.1, presentation.orbitSpeed * modifiers.orbitSpeedMult);
  return `${(BASE_ORBIT_SECONDS / speed).toFixed(3)}s`;
}

function pulseScale(mood: Mood, modifiers: PersonalityModifiers): string {
  const presentation = getMoodPresentation(mood);
  return (1 + 0.025 * presentation.pulse * modifiers.pulseMult).toFixed(3);
}

function pointOnInnerOrbit(timeSeconds: number, phaseOffset: number): PetPoint {
  const phase = timeSeconds * Math.PI * 2 + phaseOffset;
  return {
    cx: 100 + Math.cos(phase) * 20,
    cy: 110 + Math.sin(phase) * 6,
  };
}

export function getTauriPetFrameState(
  mood: Mood,
  elapsedMs: number,
  reactionElapsedMs: number,
  pressed: boolean,
  modifiers: PersonalityModifiers = DEFAULT_TAURI_MODIFIERS,
): TauriPetFrameState {
  const presentation = getMoodPresentation(mood);
  const elapsedSeconds = elapsedMs / 1000;
  const orbitSpeed = Math.max(0.1, presentation.orbitSpeed * modifiers.orbitSpeedMult);
  const pulseMult = presentation.pulse * modifiers.pulseMult;
  const floatAmplitude = mood === 'sleepy' ? 2 : 3;
  const floatY = Math.sin(elapsedSeconds * Math.PI * 2 / (mood === 'sleepy' ? 4.6 : 3.2)) * -floatAmplitude;
  const pulse = 1 + Math.sin(elapsedSeconds * 1.4) * 0.025 * pulseMult;
  const corePulse = Math.sin(elapsedSeconds * 1.8);
  const innerPulse = Math.sin(elapsedSeconds * 2.2);
  const orbitBase = elapsedSeconds * orbitSpeed * 360 / BASE_ORBIT_SECONDS;
  const reactionActive = Number.isFinite(reactionElapsedMs) && reactionElapsedMs < 700;
  const reactionProgress = reactionActive ? Math.max(0, Math.min(1, reactionElapsedMs / 620)) : 1;
  const bodyScaleX = pressed ? 1.12 : reactionActive ? 1 + Math.sin((1 - reactionProgress) * Math.PI) * 0.12 : 1;
  const bodyScaleY = pressed ? 0.88 : reactionActive ? 1 - Math.sin((1 - reactionProgress) * Math.PI) * 0.08 : 1;

  return {
    floatY,
    glowRadius: 54 * pulse,
    glowOpacity: 0.24 + Math.sin(elapsedSeconds * 2.1) * 0.04,
    shellRadius: 38 * pulse,
    shellStrokeRadius: 34 * pulse,
    ringRadius: 30 + Math.sin(elapsedSeconds * 1.1) * 1.5,
    coreOuterRadius: 21 + corePulse * 1.1,
    coreInnerRadius: 13 + innerPulse * 0.8,
    eyeHaloRadius: 8 + Math.sin(elapsedSeconds * 2.8) * 1.5,
    starScale: mood === 'thinking' ? 1.2 : 1 + Math.sin(elapsedSeconds * 2.2) * 0.04,
    orbitBackRotation: -18 + orbitBase,
    orbitFrontRotation: 162 + orbitBase,
    innerDotA: pointOnInnerOrbit(elapsedSeconds * orbitSpeed * 1.8, 0),
    innerDotB: pointOnInnerOrbit(elapsedSeconds * orbitSpeed * 1.8, Math.PI),
    wakeOpacity: mood === 'speaking' || mood === 'happy' ? 1 : 0,
    bodyScaleX,
    bodyScaleY,
    sparks: SPARK_VECTORS.map((spark) => {
      const sparkElapsed = reactionElapsedMs - spark.delay;
      if (!Number.isFinite(sparkElapsed) || sparkElapsed < 0 || sparkElapsed > 620) {
        return { cx: 100, cy: 110, r: spark.r, opacity: 0 };
      }
      const progress = Math.max(0, Math.min(1, sparkElapsed / 620));
      return {
        cx: 100 + spark.x * progress,
        cy: 110 + spark.y * progress,
        r: spark.r * (1 - progress * 0.7),
        opacity: 0.78 * (1 - progress),
      };
    }),
  };
}

function transformAroundBody(scaleX: number, scaleY: number, floatY: number): string {
  return `translate(100 110) scale(${scaleX} ${scaleY}) translate(-100 -110) translate(0 ${floatY})`;
}

function renderStar(scale: number) {
  return (
    <path
      d="M100 96 L105 105 L114 110 L105 115 L100 124 L95 115 L86 110 L95 105 Z"
      fill="#ffffff"
      opacity="0.8"
      transform={`translate(100 110) scale(${scale}) translate(-100 -110)`}
      className="tauri-original-pet-star"
    />
  );
}

function renderEye(mood: Mood, frame: TauriPetFrameState) {
  const { eyeShape } = getMoodPresentation(mood);
  switch (eyeShape) {
    case 'focused':
      return (
        <g className="tauri-original-pet-eye tauri-original-pet-eye--focused">
          <circle cx="100" cy="110" r="3.5" fill="#ffffff" opacity="0.95" />
          <path d="M93 110 H107 M100 104 V116" stroke="#ffffff" strokeWidth="0.8" opacity="0.22" />
        </g>
      );
    case 'smiling':
      return (
        <path
          className="tauri-original-pet-eye tauri-original-pet-eye--smiling"
          d="M94 112 A6 6 0 0 1 106 112"
          fill="none"
          stroke="#ffffff"
          strokeWidth="2.5"
          opacity="0.9"
        />
      );
    case 'sleepy':
      return (
        <g className="tauri-original-pet-eye tauri-original-pet-eye--sleepy">
          <circle cx="100" cy="110" r="3" fill="#ffffff" opacity="0.45" />
          <path d="M95 112 A5.5 5.5 0 0 0 105 112" fill="none" stroke="#ffffff" strokeWidth="1" opacity="0.22" />
        </g>
      );
    case 'open':
    default:
      return (
        <g className="tauri-original-pet-eye tauri-original-pet-eye--open">
          <circle cx="100" cy="110" r="4.5" fill="#ffffff" opacity="0.9" />
          <circle cx="100" cy="110" r={frame.eyeHaloRadius} fill="none" stroke="#ffffff" strokeWidth="1" opacity="0.24" />
        </g>
      );
  }
}

export function TauriOriginalPet({
  visible,
  sizeScale,
  mood,
  reactionKey,
  pressed,
  modifiers = DEFAULT_TAURI_MODIFIERS,
}: TauriOriginalPetProps) {
  const scale = Number.isFinite(sizeScale) ? sizeScale : 1;
  const startMsRef = useRef<number | null>(null);
  const reactionStartMsRef = useRef<number | null>(null);
  const previousReactionKeyRef = useRef(reactionKey);
  const [nowMs, setNowMs] = useState(0);

  useEffect(() => {
    if (!visible) return undefined;
    let frameId = 0;
    const tick = (timestamp: number) => {
      if (startMsRef.current === null) startMsRef.current = timestamp;
      setNowMs(timestamp);
      frameId = requestAnimationFrame(tick);
    };
    frameId = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frameId);
  }, [visible]);

  useEffect(() => {
    if (previousReactionKeyRef.current === reactionKey) return;
    previousReactionKeyRef.current = reactionKey;
    reactionStartMsRef.current = nowMs;
  }, [nowMs, reactionKey]);

  const elapsedMs = startMsRef.current === null ? 0 : nowMs - startMsRef.current;
  const reactionElapsedMs = reactionStartMsRef.current === null ? Number.POSITIVE_INFINITY : nowMs - reactionStartMsRef.current;
  const frame = useMemo(
    () => getTauriPetFrameState(mood, elapsedMs, reactionElapsedMs, pressed, modifiers),
    [elapsedMs, modifiers, mood, pressed, reactionElapsedMs],
  );

  if (!visible) return null;

  const presentation = getMoodPresentation(mood);
  const petColor = colorToHex(presentation.color);
  const petGlow = colorToHex(presentation.glowColor);
  const petStyle = {
    position: 'absolute',
    left: '50%',
    top: '50%',
    width: 200,
    height: 260,
    transform: `translate(-50%, -50%) scale(${scale})`,
    transformOrigin: 'center',
    pointerEvents: 'none',
    zIndex: 1,
    '--pet-color': petColor,
    '--pet-glow': petGlow,
    '--orbit-duration': orbitDurationSeconds(mood, modifiers),
    '--pulse-scale': pulseScale(mood, modifiers),
    '--wake-opacity': String(frame.wakeOpacity),
  } as CSSProperties & Record<string, string | number>;

  return (
    <div
      className={[
        'tauri-original-pet',
        `tauri-original-pet--${mood}`,
        pressed ? 'tauri-original-pet--pressed' : '',
      ].filter(Boolean).join(' ')}
      aria-label="灵枢桌面宠物"
      data-reaction-key={reactionKey}
      style={petStyle}
    >
      <svg
        width="200"
        height="260"
        viewBox="0 0 200 260"
        aria-hidden="true"
        style={{ display: 'block' }}
      >
        <defs>
          <filter id="original-pet-glow" x="-60%" y="-60%" width="220%" height="220%">
            <feGaussianBlur stdDeviation="14" />
          </filter>
          <radialGradient id="original-pet-core" cx="50%" cy="50%" r="58%">
            <stop offset="0%" stopColor="#ffffff" stopOpacity="0.34" />
            <stop offset="100%" stopColor="#d9ecff" stopOpacity="0.14" />
          </radialGradient>
        </defs>

        <g
          className="tauri-original-pet-body"
          transform={transformAroundBody(frame.bodyScaleX, frame.bodyScaleY, frame.floatY)}
        >
          <circle
            cx="100"
            cy="110"
            r={frame.glowRadius}
            fill={petGlow}
            opacity={frame.glowOpacity}
            filter="url(#original-pet-glow)"
            className="tauri-original-pet-glow"
          />
          <ellipse
            className="tauri-original-pet-orbit-back"
            cx="100"
            cy="110"
            rx="56"
            ry="15"
            fill="none"
            stroke={petColor}
            strokeWidth="1.5"
            opacity="0.22"
            transform={`rotate(${frame.orbitBackRotation} 100 110)`}
          />
          <g className="tauri-original-pet-inner-dot-a">
            <circle cx={frame.innerDotA.cx} cy={frame.innerDotA.cy} r="2" fill="#ffffff" opacity="0.28" />
          </g>
          <g className="tauri-original-pet-inner-dot-b">
            <circle cx={frame.innerDotB.cx} cy={frame.innerDotB.cy} r="2" fill="#ffffff" opacity="0.18" />
          </g>
          <g className="tauri-original-pet-pulse">
            <circle cx="100" cy="110" r={frame.shellRadius} fill={petColor} opacity="0.3" />
            <circle cx="100" cy="110" r={frame.shellStrokeRadius} fill="none" stroke="#ffffff" strokeWidth="1.5" opacity="0.38" />
            <circle cx="100" cy="110" r={frame.ringRadius} fill="none" stroke={petColor} strokeWidth="2" opacity="0.42" />
            <circle cx="100" cy="110" r={frame.coreOuterRadius} fill="url(#original-pet-core)" className="tauri-original-pet-core" />
            <circle cx="100" cy="110" r={frame.coreInnerRadius} fill={petGlow} opacity="0.34" className="tauri-original-pet-core-inner" />
            {renderStar(frame.starScale)}
            {renderEye(mood, frame)}
          </g>
          <g className="tauri-original-pet-wake" opacity={frame.wakeOpacity}>
            <path d="M60 91 A44 44 0 0 1 140 91" fill="none" stroke="#ffffff" strokeWidth="2" opacity="0.28" />
            <path d="M58 129 A47 47 0 0 0 142 129" fill="none" stroke="#ffffff" strokeWidth="2" opacity="0.18" />
          </g>
          <ellipse
            className="tauri-original-pet-orbit-front"
            cx="100"
            cy="110"
            rx="56"
            ry="15"
            fill="none"
            stroke={petColor}
            strokeWidth="2.2"
            opacity="0.52"
            transform={`rotate(${frame.orbitFrontRotation} 100 110)`}
          />
          <g className="tauri-original-pet-burst" aria-hidden="true">
            {frame.sparks.map((spark, index) => (
              <circle
                // eslint-disable-next-line react/no-array-index-key
                key={index}
                className={`tauri-original-pet-spark tauri-original-pet-spark-${index + 1}`}
                cx={spark.cx}
                cy={spark.cy}
                r={spark.r}
                fill={petGlow}
                opacity={spark.opacity}
              />
            ))}
          </g>
          <text
            x="100"
            y="170"
            textAnchor="middle"
            fontSize="12"
            fontWeight="600"
            fontFamily="system-ui, sans-serif"
            fill={petColor}
          >
            灵枢
          </text>
        </g>
      </svg>
      <style>{`
        .tauri-original-pet-wake {
          transition: opacity 180ms ease;
        }

        .tauri-original-pet-burst {
          pointer-events: none;
        }
      `}</style>
    </div>
  );
}
