import { cn } from '../../lib/cn.ts';

/**
 * Pill switch with a clearly outlined off state — the bare knob-on-a-faint-track
 * toggles read as invisible on the dark surfaces, so the off track carries a
 * `border-strong` outline and a distinct fill, and the knob a drop shadow.
 */
export function Switch({
  checked,
  onChange,
  disabled,
  id,
  ariaLabel,
}: {
  checked: boolean;
  onChange: (next: boolean) => void;
  disabled?: boolean;
  id?: string;
  ariaLabel?: string;
}) {
  return (
    <button
      type="button"
      id={id}
      role="switch"
      aria-checked={checked}
      aria-label={ariaLabel}
      onClick={() => onChange(!checked)}
      disabled={disabled}
      className={cn(
        'relative inline-flex h-6 w-11 shrink-0 cursor-pointer items-center rounded-full border transition-colors duration-200',
        'focus-visible:outline-none ',
        'disabled:cursor-not-allowed disabled:opacity-40',
        checked
          ? 'border-transparent bg-status-success'
          : 'border-transparent bg-[var(--color-border)]',
      )}
    >
      <span
        className={cn(
          'inline-block h-4 w-4 rounded-full bg-white shadow-[0_1px_2px_rgb(0_0_0/0.35)] transition-transform duration-200',
          checked ? 'translate-x-[22px]' : 'translate-x-[3px]',
        )}
      />
    </button>
  );
}
