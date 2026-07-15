import { useEffect, useRef, type RefObject } from 'react';

export function useClickOutside<T extends HTMLElement>(
  ref: RefObject<T | null>,
  enabled: boolean,
  onOutside: () => void,
  defer = false,
  relatedRef?: RefObject<HTMLElement | null>,
): void {
  const onOutsideRef = useRef(onOutside);

  useEffect(() => {
    onOutsideRef.current = onOutside;
  }, [onOutside]);

  useEffect(() => {
    if (!enabled) return;

    const handler = (event: MouseEvent) => {
      const target = event.target as Node;
      const insidePrimary = ref.current?.contains(target) ?? false;
      const insideRelated = relatedRef?.current?.contains(target) ?? false;
      if (!insidePrimary && !insideRelated) {
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
  }, [defer, enabled, ref, relatedRef]);
}
