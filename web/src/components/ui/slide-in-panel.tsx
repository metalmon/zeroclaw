import { useEffect, useRef, type ReactNode, type TransitionEvent } from "react";

import { cn } from "../../lib/cn.ts";

// Ported from Thunderbolt (fork/rebrand src/components/slide-in-panel.tsx):
// an in-flow right column that animates its width 0 -> width, so the list
// beside it shrinks as the panel opens (a real layout column, not an overlay).
const slideEasing = "cubic-bezier(0.32, 0.72, 0, 1)";

export function SlideInPanel({
  open,
  width,
  className,
  onCloseComplete,
  children,
}: {
  open: boolean;
  width: string;
  className?: string;
  onCloseComplete?: () => void;
  children: ReactNode;
}) {
  const previousOpenRef = useRef(open);

  useEffect(() => {
    const wasOpen = previousOpenRef.current;
    previousOpenRef.current = open;
    // Guard matchMedia: it is absent in the node:test harness (no jsdom) and in SSR.
    const reduced =
      typeof window !== "undefined" &&
      window.matchMedia?.("(prefers-reduced-motion: reduce)").matches === true;
    if (wasOpen && !open && reduced) {
      onCloseComplete?.();
    }
  }, [onCloseComplete, open]);

  const handleTransitionEnd = (event: TransitionEvent<HTMLElement>) => {
    if (!open && event.target === event.currentTarget) {
      onCloseComplete?.();
    }
  };

  return (
    <aside
      data-slot="slide-in-panel"
      className={cn(
        "relative z-30 h-full shrink-0 overflow-hidden transition-[width] duration-300 motion-reduce:transition-none",
        className,
      )}
      style={{ width: open ? width : "0px", transitionTimingFunction: slideEasing }}
      aria-hidden={!open}
      inert={!open}
      onTransitionEnd={handleTransitionEnd}
    >
      <div
        className="h-full transition-transform duration-300 motion-reduce:transition-none"
        style={{
          width,
          transform: open ? "translateX(0)" : "translateX(100%)",
          transitionTimingFunction: slideEasing,
        }}
      >
        {children}
      </div>
    </aside>
  );
}
