import { useEffect, useMemo, useRef, useState } from 'react';
import { X, Settings, Sun, Moon, Laptop, BookOpen, Check, Type, CaseSensitive } from 'lucide-react';
import { useTheme } from '@/hooks/useTheme';
import { t } from '@/lib/i18n';
import type { UiFont, MonoFont, ThemeMode } from '@/contexts/ThemeContext';
import { uiFontStacks, monoFontStacks } from '@/contexts/ThemeContext';

// The Volt theme set — a fixed brand palette shared with the desktop client.
// Each swatch previews the theme's real page background, foreground and brand
// accent (the same colors ThemeContext applies).
const themeOptions: {
  value: ThemeMode;
  icon: typeof Sun;
  labelKey: string;
  bg: string;
  fg: string;
  accent: string;
}[] = [
  { value: 'system', icon: Laptop, labelKey: 'theme.system', bg: 'linear-gradient(135deg, #1b1e1f 50%, #f5f2ee 50%)', fg: '#cbc6bf', accent: '#c12d6c' },
  { value: 'dark', icon: Moon, labelKey: 'theme.dark', bg: '#1b1e1f', fg: '#f2f1ee', accent: '#c12d6c' },
  { value: 'light', icon: Sun, labelKey: 'theme.light', bg: '#f5f2ee', fg: '#262120', accent: '#c12d6c' },
  { value: 'paper', icon: BookOpen, labelKey: 'theme.paper', bg: '#f0e9dd', fg: '#2b2622', accent: '#c96442' },
];

const uiFontOptions: { value: UiFont; label: string; sample: string }[] = [
  { value: 'system', label: 'System', sample: 'Segoe/UI' },
  { value: 'inter', label: 'Inter', sample: 'Inter' },
  { value: 'segoe', label: 'Segoe UI', sample: 'Segoe' },
  { value: 'sf', label: 'SF Pro', sample: 'SF' },
];

const monoFontOptions: { value: MonoFont; label: string; sample: string }[] = [
  { value: 'jetbrains', label: 'JetBrains Mono', sample: 'JetBrains' },
  { value: 'fira', label: 'Fira Code', sample: 'Fira' },
  { value: 'cascadia', label: 'Cascadia Code', sample: 'Cascadia' },
  { value: 'system-mono', label: 'System mono', sample: 'System' },
];

const uiSizes = [14, 15, 16, 17, 18];
const monoSizes = [13, 14, 15, 16, 17];

// Shared selectable-chip classes. Hover is pure CSS (no JS handlers): the
// inactive state lifts to `--pc-hover` on hover; the active state is an
// accent-tinted token surface. Both carry a strong focus-visible ring.
const chipBase =
  'border transition-colors duration-150 focus-visible:outline-none ' +
  'focus-visible:ring-2 focus-visible:ring-[var(--pc-focus)] ' +
  'focus-visible:ring-offset-2 focus-visible:ring-offset-pc-base cursor-pointer';
const chipInactive =
  'border-pc-border text-pc-text-muted bg-transparent ' +
  'hover:bg-[var(--pc-hover)] hover:text-pc-text';
const chipActive =
  'border-pc-accent-dim bg-pc-accent/10 text-pc-accent-light';

function chip(active: boolean, extra = '') {
  return [chipBase, active ? chipActive : chipInactive, extra].filter(Boolean).join(' ');
}

function SectionTitle({ children }: { children: React.ReactNode }) {
  return (
    <div className="text-[10px] uppercase tracking-wider font-semibold mb-2 mt-5 first:mt-0 text-pc-text-faint">
      {children}
    </div>
  );
}

interface Props {
  open: boolean;
  onClose: () => void;
}

export function SettingsModal({ open, onClose }: Props) {
  const {
    theme, uiFont, monoFont, uiFontSize, monoFontSize,
    setTheme, setUiFont, setMonoFont, setUiFontSize, setMonoFontSize,
  } = useTheme();

  type TabId = 'appearance' | 'typography';
  const [tab, setTab] = useState<TabId>('appearance');

  const panelRef = useRef<HTMLDivElement>(null);

  const tabs: { id: TabId; label: string; icon: typeof Settings }[] = useMemo(() => [
    { id: 'appearance', label: t('settings.tab.appearance'), icon: Settings },
    { id: 'typography', label: t('settings.tab.typography'), icon: Type },
  ], []);

  // Focus management: focus the first control on open, restore focus to the
  // trigger on close.
  useEffect(() => {
    if (!open) return;
    const previouslyFocused = document.activeElement as HTMLElement | null;
    const panel = panelRef.current;
    const firstFocusable = panel?.querySelector<HTMLElement>(
      'a[href], button:not([disabled]), input:not([disabled]), [tabindex]:not([tabindex="-1"])',
    );
    firstFocusable?.focus();
    return () => previouslyFocused?.focus?.();
  }, [open]);

  // Esc closes; Tab is trapped within the modal panel.
  useEffect(() => {
    if (!open) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        onClose();
        return;
      }
      if (e.key !== 'Tab') return;
      const panel = panelRef.current;
      if (!panel) return;
      const focusable = Array.from(
        panel.querySelectorAll<HTMLElement>(
          'a[href], button:not([disabled]), input:not([disabled]), [tabindex]:not([tabindex="-1"])',
        ),
      ).filter((el) => el.offsetParent !== null || el === document.activeElement);
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      if (!first || !last) return;
      const active = document.activeElement;
      if (e.shiftKey && active === first) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && active === last) {
        e.preventDefault();
        first.focus();
      }
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [open, onClose]);

  if (!open) return null;

  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-label={t('settings.title')}
      className="fixed inset-0 z-50 flex items-center justify-center"
      onClick={onClose}
    >
      <div className="absolute inset-0 bg-pc-base/70 backdrop-blur-sm" />
      <div
        ref={panelRef}
        className="relative flex flex-col w-full max-w-2xl mx-4 max-h-[90vh] rounded-[var(--radius-xl)] border border-pc-border bg-pc-base shadow-[var(--pc-shadow-md)] animate-fade-in"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div className="flex-shrink-0 flex items-center justify-between px-6 py-4 border-b border-pc-border">
          <div className="flex items-center gap-2.5">
            <Settings size={18} className="text-pc-accent-light" />
            <h2 className="text-sm font-semibold text-pc-text">{t('settings.title')}</h2>
          </div>
          <button
            type="button"
            onClick={onClose}
            aria-label={t('common.close')}
            className="h-11 w-11 -mr-2 rounded-[var(--radius-md)] flex items-center justify-center text-pc-text-muted transition-colors hover:bg-[var(--pc-hover)] hover:text-pc-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--pc-focus)] focus-visible:ring-offset-2 focus-visible:ring-offset-pc-base"
          >
            <X size={16} />
          </button>
        </div>

        {/* Body */}
        <div className="flex-1 min-h-0 px-6 py-4 overflow-y-auto">
          {/* Tabs */}
          <div className="flex gap-2 mb-4">
            {tabs.map(tTab => (
              <button
                key={tTab.id}
                type="button"
                onClick={() => setTab(tTab.id)}
                className={chip(
                  tab === tTab.id,
                  'flex-1 rounded-[var(--radius-md)] px-3 py-2 text-xs font-medium flex items-center justify-center gap-1.5',
                )}
                aria-pressed={tab === tTab.id}
              >
                <tTab.icon size={13} />
                {tTab.label}
              </button>
            ))}
          </div>

          {/* Appearance Tab — the Volt theme picker. */}
          {tab === 'appearance' && (
            <>
              <SectionTitle>{t('settings.appearance')}</SectionTitle>
              <div className="text-xs mb-2 text-pc-text-secondary">{t('theme.mode')}</div>
              <div className="grid grid-cols-2 sm:grid-cols-4 gap-2">
                {themeOptions.map(opt => {
                  const Icon = opt.icon;
                  const active = theme === opt.value;
                  return (
                    <button
                      key={opt.value}
                      type="button"
                      onClick={() => setTheme(opt.value)}
                      aria-pressed={active}
                      className={[
                        'flex flex-col gap-2 p-2 rounded-[var(--radius-lg)] border text-left',
                        'transition-colors duration-150 cursor-pointer',
                        'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--pc-focus)]',
                        'focus-visible:ring-offset-2 focus-visible:ring-offset-pc-base',
                        active
                          ? 'border-pc-accent bg-pc-accent/10'
                          : 'border-pc-border hover:bg-[var(--pc-hover)] hover:border-pc-border-strong',
                      ].join(' ')}
                    >
                      {/* Mini preview — the theme's literal page colors. */}
                      <div
                        className="relative w-full h-12 rounded-md overflow-hidden border"
                        style={{
                          background: opt.bg,
                          borderColor: opt.value === 'dark' || opt.value === 'system' ? 'rgba(255,255,255,0.12)' : 'rgba(0,0,0,0.10)',
                        }}
                      >
                        <Icon size={12} style={{ color: opt.fg }} className="absolute top-1.5 left-1.5" />
                        {/* Brand accent dot + text lines. */}
                        <span className="absolute bottom-1.5 left-1.5 h-2 w-2 rounded-full" style={{ background: opt.accent }} />
                        <span className="absolute bottom-2 left-4 h-[3px] w-6 rounded-full" style={{ background: opt.fg, opacity: 0.35 }} />
                      </div>
                      <div className="flex items-center gap-1 px-0.5">
                        {active && <Check size={11} className="text-pc-accent" />}
                        <span className={['text-xs font-medium', active ? 'text-pc-accent-light' : 'text-pc-text-secondary'].join(' ')}>
                          {t(opt.labelKey)}
                        </span>
                      </div>
                    </button>
                  );
                })}
              </div>
            </>
          )}

          {/* Typography Tab */}
          {tab === 'typography' && (
            <>
              <SectionTitle>{t('settings.typography')}</SectionTitle>

              {/* UI Font */}
              <div className="mb-4">
                <div className="flex items-center gap-2 text-xs mb-2 text-pc-text-secondary">
                  <Type size={14} />
                  {t('settings.fontUi')}
                </div>
                <div className="flex flex-wrap gap-1.5">
                  {uiFontOptions.map(opt => (
                    <button
                      key={opt.value}
                      type="button"
                      onClick={() => setUiFont(opt.value)}
                      className={chip(
                        uiFont === opt.value,
                        'flex items-center gap-2 px-3 py-2 rounded-[var(--radius-md)] text-xs',
                      )}
                      aria-pressed={uiFont === opt.value}
                    >
                      <span style={{ fontSize: '14px', fontFamily: uiFontStacks[opt.value] }}>{opt.sample}</span>
                      <span className="text-pc-text-faint" style={{ fontSize: '11px' }}>{opt.label}</span>
                    </button>
                  ))}
                </div>
              </div>

              {/* Mono Font */}
              <div className="mb-4">
                <div className="flex items-center gap-2 text-xs mb-2 text-pc-text-secondary">
                  <CaseSensitive size={14} />
                  {t('settings.fontMono')}
                </div>
                <div className="flex flex-wrap gap-1.5">
                  {monoFontOptions.map(opt => (
                    <button
                      key={opt.value}
                      type="button"
                      onClick={() => setMonoFont(opt.value)}
                      className={chip(
                        monoFont === opt.value,
                        'flex items-center gap-2 px-3 py-2 rounded-[var(--radius-md)] text-xs',
                      )}
                      aria-pressed={monoFont === opt.value}
                    >
                      <span style={{ fontSize: '14px', fontFamily: monoFontStacks[opt.value] }}>{opt.sample}</span>
                      <span className="text-pc-text-faint" style={{ fontSize: '11px' }}>{opt.label}</span>
                    </button>
                  ))}
                </div>
              </div>

              {/* UI Font Size */}
              <div className="mb-4">
                <div className="text-xs mb-2 text-pc-text-secondary">{t('settings.fontSize')}</div>
                <div className="flex gap-1.5 flex-wrap">
                  {uiSizes.map(size => (
                    <button
                      key={size}
                      type="button"
                      onClick={() => setUiFontSize(size)}
                      className={chip(
                        uiFontSize === size,
                        'px-3 py-1.5 rounded-[var(--radius-md)] text-xs',
                      )}
                      aria-pressed={uiFontSize === size}
                    >
                      {size}px
                    </button>
                  ))}
                </div>
              </div>

              {/* Mono Font Size */}
              <div className="mb-4">
                <div className="text-xs mb-2 text-pc-text-secondary">{t('settings.fontMonoSize')}</div>
                <div className="flex gap-1.5 flex-wrap">
                  {monoSizes.map(size => (
                    <button
                      key={size}
                      type="button"
                      onClick={() => setMonoFontSize(size)}
                      className={chip(
                        monoFontSize === size,
                        'px-3 py-1.5 rounded-[var(--radius-md)] text-xs',
                      )}
                      aria-pressed={monoFontSize === size}
                    >
                      {size}px
                    </button>
                  ))}
                </div>
              </div>

              {/* Preview */}
              <div className="rounded-[var(--radius-lg)] border border-pc-border bg-pc-surface p-3">
                <div className="text-[11px] uppercase tracking-wide mb-2 text-pc-text-faint">
                  {t('settings.preview')}
                </div>
                <div
                  className="text-sm mb-2 text-pc-text"
                  style={{ fontFamily: 'var(--pc-font-ui)', fontSize: 'var(--pc-font-size)' }}
                >
                  {t('settings.previewText')}
                </div>
                <div
                  className="rounded-[var(--radius-md)] border border-pc-border bg-pc-code p-2 text-[13px] text-pc-text"
                  style={{ fontFamily: 'var(--pc-font-mono)', fontSize: 'var(--pc-font-size-mono)' }}
                >
                  const hello = 'Volt Agent'; // typography preview
                </div>
              </div>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
