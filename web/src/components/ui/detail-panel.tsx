import type { ReactNode } from 'react';
import { X } from 'lucide-react';

import { t } from '@/lib/i18n';
import { cn } from '../../lib/cn.ts';
import { Button, mutedIconButtonClass } from './Button.tsx';
import { SlideInPanel } from './slide-in-panel.tsx';
import { panelFieldSurfaceClass } from './modal-styles.ts';

// Ported from Thunderbolt (fork/rebrand src/components/detail-panel.tsx),
// desktop path only (the mobile responsive-modal branch is dropped — this
// panel is a desktop admin surface). Classes are kept verbatim so Thunderbolt
// design diffs port straight across.

export function DetailSectionTitle({ children }: { children: ReactNode }) {
  return (
    <h3 className="flex items-center gap-1.5 text-xs font-medium uppercase tracking-wide text-muted-foreground">
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
    <section className="relative flex h-full flex-1 flex-col overflow-hidden px-4 text-foreground md:px-6 md:pb-5">
      <header className="relative mt-2.5 flex h-16 shrink-0 items-center justify-between gap-4">
        <div className="flex min-w-0 items-center gap-3">
          {icon}
          <div className="flex min-w-0 flex-col justify-center leading-tight">
            <h2 className="min-w-0 truncate text-xl leading-tight text-foreground">{title}</h2>
            {subtitle && <span className="truncate text-xs text-muted-foreground">{subtitle}</span>}
          </div>
        </div>
        <div className="absolute -right-4 -top-0.5 flex shrink-0 items-center gap-0.5">
          {actions}
          <Button
            variant="ghost"
            size="icon"
            onClick={onClose}
            aria-label={t('detail.close')}
            className={mutedIconButtonClass}
          >
            <X />
          </Button>
        </div>
      </header>
      <div className="flex min-h-0 flex-1 flex-col gap-5 overflow-y-auto md:pt-4">{children}</div>
    </section>
  );
}

interface DetailPanelSurfaceProps {
  open: boolean;
  onCloseComplete?: () => void;
  topInset?: boolean;
  children: ReactNode;
}

export function DetailPanelSurface({ open, onCloseComplete, topInset = false, children }: DetailPanelSurfaceProps) {
  return (
    <SlideInPanel
      open={open}
      onCloseComplete={onCloseComplete}
      width="clamp(var(--create-panel-min-width), calc(50vw - 128px), 540px)"
    >
      <div className={cn('h-full pb-12', topInset && 'pt-12')}>
        <div
          className={cn(
            'h-full overflow-hidden rounded-l-2xl border border-r-0 border-border/60 bg-sidebar',
            panelFieldSurfaceClass,
          )}
        >
          {children}
        </div>
      </div>
    </SlideInPanel>
  );
}
