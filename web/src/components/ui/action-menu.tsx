import { useEffect, useRef, useState, type ReactNode } from 'react';
import { MoreVertical } from 'lucide-react';

import { cn } from '../../lib/cn.ts';
import { t } from '@/lib/i18n';
import { Button, mutedIconButtonClass } from './Button.tsx';

export interface ActionMenuItem {
  label: string;
  icon?: ReactNode;
  onClick: () => void;
  danger?: boolean;
}

/**
 * The `⋯` overflow menu Thunderbolt puts in a detail panel's header — a ghost
 * icon button that opens a small dropdown of actions (e.g. Delete), instead of
 * exposing a destructive button next to the close control.
 */
export function ActionMenu({ items, ariaLabel }: { items: ActionMenuItem[]; ariaLabel?: string }) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onDoc = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener('mousedown', onDoc);
    return () => document.removeEventListener('mousedown', onDoc);
  }, [open]);

  return (
    <div className="relative" ref={ref}>
      <Button
        variant="ghost"
        size="icon"
        aria-label={ariaLabel ?? t('common.more')}
        aria-expanded={open}
        className={mutedIconButtonClass}
        onClick={() => setOpen((v) => !v)}
      >
        <MoreVertical />
      </Button>
      {open && (
        <div className="absolute right-0 top-[calc(100%+4px)] z-50 w-52 rounded-xl border border-border bg-card p-1 shadow-md">
          {items.map((item, i) => (
            <button
              key={i}
              type="button"
              onClick={() => {
                setOpen(false);
                item.onClick();
              }}
              className={cn(
                'flex w-full items-center gap-2.5 rounded-lg px-2.5 py-2 text-left text-sm transition-colors [&_svg]:h-[17px] [&_svg]:w-[17px] [&_svg]:shrink-0',
                item.danger
                  ? 'text-status-error hover:bg-status-error/10'
                  : 'text-sidebar-foreground hover:bg-accent hover:text-accent-foreground',
              )}
            >
              {item.icon}
              {item.label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
