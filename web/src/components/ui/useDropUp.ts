import { useLayoutEffect, useState, type RefObject } from "react";

/**
 * Decide whether an opened dropdown list should flip ABOVE its trigger.
 *
 * The list is rendered `absolute` inside its own panel, so it cannot escape
 * the panel's stacking context (`surface-panel` uses `backdrop-filter`) and a
 * later sibling such as the config editor's sticky save bar paints over it.
 * Instead of guessing footer heights, hit-test the list's bottom edge once
 * after it mounts: if that point is off-screen or lands on an element outside
 * the list, something covers it and the list opens upward.
 */
export function useDropUp(open: boolean, listRef: RefObject<HTMLElement | null>): boolean {
  const [above, setAbove] = useState(false);
  useLayoutEffect(() => {
    if (!open) {
      setAbove(false);
      return;
    }
    const ul = listRef.current;
    if (!ul) return;
    const r = ul.getBoundingClientRect();
    const hit = document.elementFromPoint(r.left + 8, r.bottom - 4);
    const covered = r.bottom > window.innerHeight || (hit !== null && !ul.contains(hit));
    if (covered && r.top > window.innerHeight - r.bottom) setAbove(true);
  }, [open, listRef]);
  return above;
}
