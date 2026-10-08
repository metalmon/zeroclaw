import { useEffect, useRef, useState } from 'react';
import { Check, Copy } from 'lucide-react';
import { cn } from '@/lib/cn';
import { t } from '@/lib/i18n';

/** Copy text to the clipboard; falls back to a hidden textarea + execCommand
 *  for WebViews and non-secure contexts where `navigator.clipboard` is absent. */
async function copyText(text: string): Promise<boolean> {
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text);
      return true;
    }
  } catch {
    // fall through to the legacy path
  }
  try {
    const ta = document.createElement('textarea');
    ta.value = text;
    ta.setAttribute('readonly', '');
    ta.style.position = 'fixed';
    ta.style.opacity = '0';
    document.body.appendChild(ta);
    ta.select();
    const ok = document.execCommand('copy');
    document.body.removeChild(ta);
    return ok;
  } catch {
    return false;
  }
}

/**
 * Pairing code display with a copy button. Short codes (6-8 chars, the
 * classic numeric style) keep the big letter-spaced glyphs; the 32-character
 * alphanumeric codes voltd issues render smaller, wrap anywhere, and are
 * visually grouped in blocks of four with margins (not inserted characters),
 * so selecting/copying the text still yields the raw code.
 */
export function PairingCode({ code, className }: { code: string; className?: string }) {
  const long = code.length > 8;
  const groups = long ? (code.match(/.{1,4}/g) ?? [code]) : [code];
  const [copied, setCopied] = useState(false);
  const timer = useRef<number | null>(null);
  useEffect(() => () => { if (timer.current) window.clearTimeout(timer.current); }, []);

  const onCopy = async () => {
    if (!(await copyText(code))) return;
    setCopied(true);
    if (timer.current) window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => setCopied(false), 1500);
  };

  const label = copied ? t('pairing.copied') : t('pairing.copy_code');
  return (
    <div className={cn('flex items-center justify-center gap-2', className)}>
      <div
        className={cn(
          'min-w-0 font-mono font-bold text-foreground [overflow-wrap:anywhere] break-all',
          long ? 'text-xl leading-relaxed tracking-[0.08em]' : 'text-4xl tracking-[0.4em]',
        )}
      >
        {groups.map((g, i) => (
          <span key={i} className={long && i < groups.length - 1 ? 'mr-[0.45em]' : undefined}>
            {g}
          </span>
        ))}
      </div>
      <button
        type="button"
        onClick={() => void onCopy()}
        aria-label={label}
        title={label}
        aria-live="polite"
        className={cn(
          'flex h-8 w-8 shrink-0 items-center justify-center rounded-[var(--radius-md)] border border-border transition-colors',
          'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/50',
          copied ? 'text-status-success' : 'text-muted-foreground hover:bg-secondary/60 hover:text-foreground',
        )}
      >
        {copied ? <Check className="h-4 w-4" /> : <Copy className="h-4 w-4" />}
      </button>
    </div>
  );
}
