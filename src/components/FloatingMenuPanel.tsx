import { useLayoutEffect, useState, type CSSProperties, type ReactNode, type RefObject } from 'react';
import { createPortal } from 'react-dom';

interface FloatingMenuPanelProps<T extends HTMLElement> {
  anchorRef: RefObject<T | null>;
  panelRef: RefObject<HTMLDivElement | null>;
  children: ReactNode;
  style: CSSProperties;
}

interface MenuPosition {
  top: number;
  right: number;
}

const MENU_GAP = 8;

export default function FloatingMenuPanel<T extends HTMLElement>({
  anchorRef,
  panelRef,
  children,
  style,
}: FloatingMenuPanelProps<T>) {
  const [position, setPosition] = useState<MenuPosition | null>(null);

  useLayoutEffect(() => {
    const updatePosition = () => {
      const anchor = anchorRef.current;
      if (!anchor) return;

      const rect = anchor.getBoundingClientRect();
      setPosition({
        top: rect.bottom + MENU_GAP,
        right: Math.max(MENU_GAP, window.innerWidth - rect.right),
      });
    };

    updatePosition();
    window.addEventListener('resize', updatePosition);
    return () => window.removeEventListener('resize', updatePosition);
  }, [anchorRef]);

  const portalTarget = document.querySelector<HTMLElement>('.app-root');
  if (!portalTarget || !position) return null;

  return createPortal(
    <div
      ref={panelRef}
      className="glass-menu-panel"
      style={{
        ...style,
        position: 'fixed',
        top: `${position.top}px`,
        right: `${position.right}px`,
      }}
    >
      {children}
    </div>,
    portalTarget,
  );
}
