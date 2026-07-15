import { useEffect, useRef, type RefObject } from 'react';

export function useInfiniteScroll(
  rootRef: RefObject<HTMLElement | null>,
  targetRef: RefObject<HTMLElement | null>,
  enabled: boolean,
  onLoadMore: () => void,
): void {
  const onLoadMoreRef = useRef(onLoadMore);

  useEffect(() => {
    onLoadMoreRef.current = onLoadMore;
  }, [onLoadMore]);

  useEffect(() => {
    if (!enabled || !rootRef.current || !targetRef.current) return;

    const observer = new IntersectionObserver(
      ([entry]) => {
        if (entry.isIntersecting) onLoadMoreRef.current();
      },
      { root: rootRef.current, rootMargin: '180px 0px' },
    );
    observer.observe(targetRef.current);
    return () => observer.disconnect();
  }, [enabled, rootRef, targetRef]);
}
