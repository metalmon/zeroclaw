import type { ReactNode } from 'react';

import { cn } from '../../lib/cn.ts';

// Ported from Thunderbolt (fork/rebrand src/components/settings/icon-tile.tsx):
// the leading square tile that carries a row's / detail header's icon.
export function IconTile({ className, children }: { className?: string; children: ReactNode }) {
  return (
    <div
      className={cn(
        'flex aspect-square size-9 shrink-0 items-center justify-center overflow-hidden rounded-md bg-muted text-foreground',
        className,
      )}
    >
      {children}
    </div>
  );
}
