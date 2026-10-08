import type { ReactNode } from 'react';
import { cn } from '../../lib/cn.ts';
import { Card } from './Card';
import { IconTile } from './icon-tile';

/**
 * First-class empty state: a dashed surface with a branded icon tile, a
 * title, an optional hint line, and an optional call-to-action. Mirrors the
 * Thunderbolt "nothing here yet" treatment used across the settings lists.
 */
export function EmptyState({
  icon,
  title,
  hint,
  action,
  className,
}: {
  icon: ReactNode;
  title: string;
  hint?: string;
  action?: ReactNode;
  className?: string;
}) {
  return (
    <Card padded={false} className={cn('border-dashed p-12 text-center', className)}>
      <IconTile className="mx-auto mb-4 size-12 bg-primary/10 text-primary">{icon}</IconTile>
      <p className="mb-1 text-base font-medium text-foreground">{title}</p>
      {hint ? <p className={cn('text-sm text-muted-foreground', action && 'mb-4')}>{hint}</p> : null}
      {action}
    </Card>
  );
}
