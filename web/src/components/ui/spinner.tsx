import { type ComponentProps, useEffect, useId, useState } from 'react';
import { cn } from '../../lib/cn.ts';

/** The Volt logo bolt silhouette (from Thunderbolt `assets/logo.svg`), in a 0 0 32 32 box. */
const BOLT_PATH =
  'M23.296 1.48 13.172 7.44a.5.5 0 0 0-.191.205L7.904 17.852a.444.444 0 0 0 .398.642h4.458L7.477 29.645c-.12.254.209.48.402.276l15.849-16.735c.566-.598.142-1.583-.681-1.583h-4.282l4.945-9.734c.134-.263-.16-.54-.414-.389Z';

/** Small local replacement for framer-motion's `useReducedMotion` (that dep isn't in this app). */
function usePrefersReducedMotion(): boolean {
  const [reduce, setReduce] = useState(false);
  useEffect(() => {
    const mq = window.matchMedia?.('(prefers-reduced-motion: reduce)');
    if (!mq) return;
    setReduce(mq.matches);
    const onChange = (e: MediaQueryListEvent) => setReduce(e.matches);
    mq.addEventListener?.('change', onChange);
    return () => mq.removeEventListener?.('change', onChange);
  }, []);
  return reduce;
}

type SpinnerProps = ComponentProps<'svg'> & {
  /** Pixel size fallback when no `size-*`/`h-* w-*` className is given. */
  size?: number;
};

/**
 * Volt loading spinner — the logo bolt standing still while a pulse of charge
 * sweeps up through it. Monochrome: the bolt inherits `currentColor`, so it
 * works in any context and reads in pure black-and-white. Size it with a
 * `size-*` / `h-* w-*` className (preferred) or the `size` prop.
 *
 * Verbatim port of Thunderbolt's `components/ui/spinner.tsx`: the charge
 * animates internally, so no `animate-spin` is needed (and the bolt must never
 * rotate — it's a recognizable mark). The charge sweep is a SMIL `<animate>` on
 * the band's `y` (SVG user units), not a CSS transform — CSS transforms on SVG
 * child elements resolve inconsistently across engines (incl. WebView2).
 */
export function Spinner({ size = 24, className, ...props }: SpinnerProps) {
  // Unique per instance so multiple spinners on a page don't collide on the clip
  // id. Strip the colons React's useId emits — a `url(#:r0:)` fragment reference
  // is unreliable across SVG renderers (incl. WebView2).
  const clipId = `volt-spin-${useId().replace(/:/g, '')}`;
  const reduce = usePrefersReducedMotion();
  const cleanedClassName =
    className?.replace(/\banimate-spin\b/g, '').replace(/\s+/g, ' ').trim() || undefined;
  return (
    <svg
      viewBox="0 0 32 32"
      width={size}
      height={size}
      className={cn('volt-spinner shrink-0', cleanedClassName)}
      aria-hidden={true}
      {...props}
    >
      <clipPath id={clipId}>
        <path d={BOLT_PATH} />
      </clipPath>
      {/* Dim bolt silhouette — always visible, so the mark reads even mid-charge. */}
      <path d={BOLT_PATH} className="fill-current opacity-20" />
      {/* Bright charge, clipped to the bolt, sweeping bottom->top on a steady beat.
          Reduced motion: hold a static half-charge instead of animating. */}
      <g clipPath={`url(#${clipId})`}>
        <rect x="0" y={reduce ? 11 : 34} width="32" height="10" className="fill-current volt-charge-band">
          {!reduce && (
            <animate
              attributeName="y"
              values="34;-14"
              dur="1.2s"
              calcMode="spline"
              keyTimes="0;1"
              keySplines="0.45 0 0.55 1"
              repeatCount="indefinite"
            />
          )}
        </rect>
      </g>
    </svg>
  );
}

/** Centred full-area loading state. Uses the muted foreground, matching TB usage. */
export function SpinnerScreen({ className }: { className?: string }) {
  return (
    <div className={cn('flex h-64 w-full items-center justify-center', className)}>
      <Spinner size={32} className="text-muted-foreground" />
    </div>
  );
}
