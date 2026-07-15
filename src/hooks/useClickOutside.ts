import { useEffect, useRef, type RefObject } from 'react';

export function useClickOutside<T extends HTMLElement>(
  ref: RefObject<T | null>,
  enabled: boolean,
  onOutside: () => void,
  defer = false,
): void {
  const onOutsideRef = useRef(onOutside);

  useEffect(() => {
    onOutsideRef.current = onOutside;
  }, [onOutside]);

  useEffect(() => {
    if (!enabled) return;

    const handler = (event: MouseEvent) => {
      if (ref.current && !ref.current.contains(event.target as Node)) {
        onOutsideRef.current();
      }
    };
    const attach = () => document.addEventListener('mousedown', handler);
    const timer = defer ? window.setTimeout(attach, 0) : undefined;
    if (!defer) attach();

    return () => {
      if (timer !== undefined) window.clearTimeout(timer);
      document.removeEventListener('mousedown', handler);
    };
  }, [defer, enabled, ref]);
}
