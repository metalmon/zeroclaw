import type { ReactNode } from 'react';
import { X } from 'lucide-react';

import { cn } from '../../lib/cn.ts';
import { t } from '@/lib/i18n';

// Ported from Thunderbolt (fork/rebrand src/components/detail-panel.tsx),
// simplified to a desktop right slide-in overlay. Editing a role/agent/skill/
// config opens here as a master-detail panel rather than a modal.

export function DetailSectionTitle({ children }: { children: ReactNode }) {
  return (
    <h3 className="mb-2.5 flex items-center gap-1.5 text-[10px] font-medium uppercase tracking-wide text-muted-foreground">
      {children}
    </h3>
  );
}

export function DetailDivider() {
  return <div className="h-px shrink-0 bg-border/60" />;
}

interface DetailPanelProps {
  icon?: ReactNode;
  title: ReactNode;
  subtitle?: ReactNode;
  actions?: ReactNode;
  onClose: () => void;
  children: ReactNode;
}

export function DetailPanel({ icon, title, subtitle, actions, onClose, children }: DetailPanelProps) {
  return (
    <section className="relative my-3 ml-3 flex min-h-[min(28rem,calc(100dvh-5.5rem))] max-h-[calc(100dvh-5.5rem)] flex-col overflow-y-auto rounded-l-2xl border-y border-l border-border bg-card px-5 pb-5 pt-3 text-foreground shadow-sm">
      <header className="relative flex min-h-12 shrink-0 items-center justify-between gap-4">
        <div className="flex min-w-0 items-center gap-3">
          {icon}
          <div className="flex min-w-0 flex-col justify-center leading-tight">
            <h2 className="min-w-0 truncate text-xl leading-tight text-foreground">{title}</h2>
            {subtitle && <span className="truncate text-xs text-muted-foreground">{subtitle}</span>}
          </div>
        </div>
        <div className="flex shrink-0 items-center gap-0.5">
          {actions}
          <button
            type="button"
            onClick={onClose}
            aria-label={t('detail.close')}
            className="grid h-8 w-8 place-items-center rounded-lg text-muted-foreground transition-colors hover:bg-accent hover:text-foreground"
          >
            <X className="h-[18px] w-[18px]" />
          </button>
        </div>
      </header>
      <div className="flex min-h-0 flex-1 flex-col pt-3">{children}</div>
    </section>
  );
}

interface DetailPanelSurfaceProps {
  open: boolean;
  onClose: () => void;
  children: ReactNode;
}

/** Right slide-in surface (Thunderbolt DetailPanelSurface): ~520px, border-l,
 *  card ground, backdrop on mobile. */
export function DetailPanelSurface({ open, onClose, children }: DetailPanelSurfaceProps) {
  return (
    <>
      <div
        className={cn(
          'fixed inset-0 z-[60] bg-black/40 backdrop-blur-sm transition-opacity md:hidden',
          open ? 'opacity-100' : 'pointer-events-none opacity-0',
        )}
        onClick={onClose}
        aria-hidden="true"
      />
      <aside
        className={cn(
          'fixed z-[61] flex flex-col overflow-y-auto bg-card border-border shadow-xl transition-transform duration-200 ease-out',
          // mobile: full-height right sheet
          'inset-y-0 right-0 w-full max-w-[92vw] border-l',
          // desktop: a panel sized to its content, floating below the title bar
          'md:inset-y-auto md:right-4 md:top-16 md:w-[520px] md:max-w-[calc(100vw-5rem)] md:max-h-[calc(100dvh-5.5rem)] md:rounded-2xl md:border',
          open ? 'translate-x-0' : 'translate-x-full',
        )}
        role="dialog"
        aria-modal="true"
      >
        {open && children}
      </aside>
    </>
  );
}
