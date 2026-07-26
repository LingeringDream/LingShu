import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { TauriOriginalPet, getTauriPetFrameState } from '../TauriOriginalPet';

describe('TauriOriginalPet', () => {
  it('renders the original pet geometry as visible DOM when enabled', () => {
    render(<TauriOriginalPet visible sizeScale={1.1} mood="idle" reactionKey={0} pressed={false} />);

    const pet = screen.getByLabelText('灵枢桌面宠物');
    expect(pet).toBeInTheDocument();
    expect(pet).toHaveStyle({ transform: 'translate(-50%, -50%) scale(1.1)' });
    expect(screen.getByText('灵枢')).toBeInTheDocument();
  });

  it('keeps the original pet animated and mood-driven in the Tauri DOM renderer', () => {
    render(<TauriOriginalPet visible sizeScale={1} mood="happy" reactionKey={0} pressed={false} />);

    const pet = screen.getByLabelText('灵枢桌面宠物');
    expect(pet).toHaveClass('tauri-original-pet--happy');
    expect(pet).toHaveStyle({
      '--pet-color': '#ffc060',
      '--pet-glow': '#ffd48a',
      '--orbit-duration': '1.867s',
    });
    expect(pet.querySelector('.tauri-original-pet-pulse')).not.toBeNull();
    expect(pet.querySelector('.tauri-original-pet-inner-dot-a')).not.toBeNull();
    expect(pet.querySelector('.tauri-original-pet-wake')).not.toBeNull();
  });

  it('exposes press and reaction animations so pet interactions are visible', () => {
    const { rerender } = render(
      <TauriOriginalPet visible sizeScale={1} mood="idle" reactionKey={1} pressed={false} />,
    );

    let pet = screen.getByLabelText('灵枢桌面宠物');
    expect(pet).toHaveAttribute('data-reaction-key', '1');
    expect(pet.querySelector('.tauri-original-pet-burst')).not.toBeNull();

    rerender(<TauriOriginalPet visible sizeScale={1} mood="idle" reactionKey={2} pressed />);

    pet = screen.getByLabelText('灵枢桌面宠物');
    expect(pet).toHaveClass('tauri-original-pet--pressed');
    expect(pet).toHaveAttribute('data-reaction-key', '2');
  });

  it('computes different SVG frame attributes over time instead of relying only on CSS animations', () => {
    const start = getTauriPetFrameState('idle', 0, 900, false);
    const later = getTauriPetFrameState('idle', 900, 0, false);

    expect(later.floatY).not.toBe(start.floatY);
    expect(later.orbitBackRotation).not.toBe(start.orbitBackRotation);
    expect(later.coreOuterRadius).not.toBe(start.coreOuterRadius);
    expect(later.innerDotA.cx).not.toBe(start.innerDotA.cx);
  });

  it('renders nothing when the pet is hidden', () => {
    render(<TauriOriginalPet visible={false} sizeScale={1} mood="idle" reactionKey={0} pressed={false} />);

    expect(screen.queryByLabelText('灵枢桌面宠物')).toBeNull();
  });
});
