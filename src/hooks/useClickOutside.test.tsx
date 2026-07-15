import { fireEvent, render, screen } from '@testing-library/react';
import { useRef } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { useClickOutside } from './useClickOutside';

function TestMenu({ onOutside }: { onOutside: () => void }) {
  const menuRef = useRef<HTMLDivElement>(null);
  useClickOutside(menuRef, true, onOutside);

  return (
    <>
      <div ref={menuRef} data-testid="menu">Menu content</div>
      <button type="button">Outside</button>
    </>
  );
}

function TestPortaledMenu({ onOutside }: { onOutside: () => void }) {
  const triggerRef = useRef<HTMLDivElement>(null);
  const panelRef = useRef<HTMLDivElement>(null);
  useClickOutside(triggerRef, true, onOutside, false, panelRef);

  return (
    <>
      <div ref={triggerRef}>Trigger</div>
      <div ref={panelRef} data-testid="floating-menu">Floating menu content</div>
      <button type="button">Outside portal</button>
    </>
  );
}

describe('useClickOutside', () => {
  it('only invokes the callback for pointer events outside the target', () => {
    const onOutside = vi.fn();
    render(<TestMenu onOutside={onOutside} />);

    fireEvent.mouseDown(screen.getByTestId('menu'));
    expect(onOutside).not.toHaveBeenCalled();

    fireEvent.mouseDown(screen.getByRole('button', { name: 'Outside' }));
    expect(onOutside).toHaveBeenCalledOnce();
  });

  it('treats a related floating panel as part of the target', () => {
    const onOutside = vi.fn();
    render(<TestPortaledMenu onOutside={onOutside} />);

    fireEvent.mouseDown(screen.getByTestId('floating-menu'));
    expect(onOutside).not.toHaveBeenCalled();

    fireEvent.mouseDown(screen.getByRole('button', { name: 'Outside portal' }));
    expect(onOutside).toHaveBeenCalledOnce();
  });
});
