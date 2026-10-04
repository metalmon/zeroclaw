import { cn } from '@/lib/cn';

/**
 * Pairing code display. Short codes (6-8 chars, the classic numeric style)
 * keep the big letter-spaced glyphs; the 32-character alphanumeric codes voltd
 * issues render smaller, wrap anywhere, and are visually grouped in blocks of
 * four with margins (not inserted characters), so selecting/copying the text
 * still yields the raw code.
 */
export function PairingCode({ code, className }: { code: string; className?: string }) {
  const long = code.length > 8;
  const groups = long ? (code.match(/.{1,4}/g) ?? [code]) : [code];
  return (
    <div
      className={cn(
        'font-mono font-bold text-foreground [overflow-wrap:anywhere] break-all',
        long ? 'text-xl leading-relaxed tracking-[0.08em]' : 'text-4xl tracking-[0.4em]',
        className,
      )}
    >
      {groups.map((g, i) => (
        <span key={i} className={long && i < groups.length - 1 ? 'mr-[0.45em]' : undefined}>
          {g}
        </span>
      ))}
    </div>
  );
}
