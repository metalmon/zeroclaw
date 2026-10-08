import type { ComponentProps, ReactNode } from 'react';

import { cn } from '../../lib/cn.ts';
import { Card } from './Card.tsx';

// Ported from Thunderbolt (fork/rebrand src/components/settings/settings-list.tsx):
// the settings surface = a narrow centred column of flat card-rows, each with a
// leading icon, a title/subtitle stack, and a trailing control. Master-detail:
// clicking a row opens the right detail drawer; the selected row is tinted.

/** Narrow centred settings column (760px), on the content ground. */
export function SettingsPageShell({ className, ...props }: ComponentProps<'section'>) {
  return (
    <section
      className={cn(
        'mx-auto flex w-full max-w-[760px] flex-col gap-3 bg-background px-5 pt-4 pb-16 text-foreground',
        className,
      )}
      {...props}
    />
  );
}

/** Scrollable stack of rows with a consistent gap. */
export function SettingsListBody({ className, ...props }: ComponentProps<'div'>) {
  return <div className={cn('flex min-h-0 flex-col gap-3', className)} {...props} />;
}

/** Faint uppercase group label above a run of rows. */
export function SettingsSectionLabel({ className, ...props }: ComponentProps<'h2'>) {
  return (
    <h2
      className={cn(
        'mt-1.5 px-0.5 text-[10px] font-medium uppercase tracking-wide text-muted-foreground',
        className,
      )}
      {...props}
    />
  );
}

type SettingsSelectableRowProps = Omit<ComponentProps<typeof Card>, 'onSelect' | 'title'> & {
  title: ReactNode;
  subtitle?: ReactNode;
  leading?: ReactNode;
  trailing?: ReactNode;
  trailingIcon?: ReactNode;
  isSelected?: boolean;
  isDimmed?: boolean;
  /** Omit for a non-interactive row (no toggle button is rendered). */
  onSelect?: () => void;
  ariaLabel: string;
};

/** A single flat card-row. Selected = tinted; hover = subtle. */
export function SettingsSelectableRow({
  title,
  subtitle,
  leading,
  trailing,
  trailingIcon,
  isSelected = false,
  isDimmed = false,
  onSelect,
  ariaLabel,
  className,
  ...props
}: SettingsSelectableRowProps) {
  const body = (
    <>
      {leading && <span className="flex shrink-0 items-center justify-center">{leading}</span>}
      <span className="min-w-0 flex-1">
        <span className={cn('block truncate text-[15px] font-medium', isDimmed && 'text-muted-foreground')}>
          {title}
        </span>
        {subtitle && (
          <span className="mt-0.5 block truncate text-xs text-muted-foreground">{subtitle}</span>
        )}
      </span>
      {trailingIcon && <span className="flex shrink-0 items-center">{trailingIcon}</span>}
    </>
  );
  const bodyClass = cn(
    'flex min-w-0 flex-1 items-center gap-3 rounded-l-[inherit] px-4 py-3 text-left',
    !trailing && 'rounded-r-[inherit] pr-4',
  );

  return (
    <Card
      padded={false}
      className={cn(
        'flex flex-row items-stretch gap-0 border-border p-0 transition-colors',
        isSelected ? 'bg-accent' : onSelect ? 'hover:bg-secondary/50' : '',
        className,
      )}
      {...props}
    >
      {onSelect ? (
        <button
          type="button"
          aria-label={ariaLabel}
          aria-pressed={isSelected}
          onClick={onSelect}
          className={cn(bodyClass, 'cursor-pointer')}
        >
          {body}
        </button>
      ) : (
        <div className={bodyClass}>{body}</div>
      )}
      {trailing && <div className="flex shrink-0 items-center rounded-r-[inherit] pr-3">{trailing}</div>}
    </Card>
  );
}
