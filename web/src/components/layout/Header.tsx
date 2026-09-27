import { useState, useRef, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  Menu,
  ChevronLeft,
  ChevronRight,
  Search,
  Settings,
  LogOut,
  Rocket,
  ChevronDown,
  User,
} from 'lucide-react';
import { t } from '@/lib/i18n';
import { useAuth } from '@/hooks/useAuth';
import { SettingsModal } from '@/components/SettingsModal';
import ReloadDaemonButton from '@/components/sections/ReloadDaemonButton';
import { Button } from '@/components/ui';

interface HeaderProps {
  onMenuToggle: () => void;
  onOpenPalette: () => void;
}

const menuItemClass =
  'flex w-full items-center gap-2.5 rounded-lg px-2.5 py-2 text-sm text-sidebar-foreground text-left transition-colors hover:bg-accent hover:text-accent-foreground [&_svg]:h-[17px] [&_svg]:w-[17px] [&_svg]:shrink-0';

/**
 * Unified title bar (Thunderbolt chrome): seamless, no repeated section title
 * and no always-visible exit/onboarding buttons. Left = menu + history; right =
 * ⌘K search + a single account menu that folds in Quickstart / Reload / Settings
 * / Logout. `data-tauri-drag-region` is inert in a browser and becomes the drag
 * surface when the panel runs inside the «Волт Админ» Tauri shell (variant B).
 */
export default function Header({ onMenuToggle, onOpenPalette }: HeaderProps) {
  const navigate = useNavigate();
  const { logout } = useAuth();
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [menuOpen, setMenuOpen] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!menuOpen) return;
    const onDoc = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        setMenuOpen(false);
      }
    };
    document.addEventListener('mousedown', onDoc);
    return () => document.removeEventListener('mousedown', onDoc);
  }, [menuOpen]);

  const handleLogout = () => {
    if (window.confirm(t('auth.logout_confirm'))) {
      logout();
    }
  };

  return (
    <>
      <header
        data-tauri-drag-region
        className="h-12 flex items-center gap-1 px-2 bg-background shrink-0 relative z-[100]"
      >
        {/* Mobile drawer toggle — desktop has the persistent rail. */}
        <Button
          variant="ghost"
          size="icon-sm"
          onClick={onMenuToggle}
          className="md:hidden text-muted-foreground hover:text-foreground"
          aria-label={t('header.open_menu')}
        >
          <Menu className="h-5 w-5" />
        </Button>

        {/* History — the Thunderbolt back/forward pair. */}
        <Button
          variant="ghost"
          size="icon-sm"
          onClick={() => navigate(-1)}
          className="hidden md:inline-flex text-muted-foreground hover:text-foreground"
          aria-label={t('header.back')}
        >
          <ChevronLeft className="h-[18px] w-[18px]" />
        </Button>
        <Button
          variant="ghost"
          size="icon-sm"
          onClick={() => navigate(1)}
          className="hidden md:inline-flex text-muted-foreground hover:text-foreground"
          aria-label={t('header.forward')}
        >
          <ChevronRight className="h-[18px] w-[18px]" />
        </Button>

        <div className="flex-1" />

        {/* ⌘K palette trigger, styled as a quiet search field. */}
        <button
          type="button"
          onClick={onOpenPalette}
          className="hidden sm:flex h-8 items-center gap-2 rounded-lg border border-border bg-transparent pl-2.5 pr-2 text-sm text-muted-foreground transition-colors hover:border-border-strong hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
          aria-label={t('nav.cmdk.placeholder')}
        >
          <Search className="h-4 w-4 shrink-0" aria-hidden="true" />
          <span className="hidden md:inline w-32 text-left truncate">{t('nav.cmdk.placeholder')}</span>
          <kbd className="ml-1 flex items-center gap-0.5 rounded-md border border-border px-1.5 py-0.5 text-[11px] font-mono text-muted-foreground">
            <span className="text-[13px] leading-none">⌘</span>K
          </kbd>
        </button>
        <Button
          variant="ghost"
          size="icon-sm"
          onClick={onOpenPalette}
          className="sm:hidden text-muted-foreground hover:text-foreground"
          aria-label={t('nav.cmdk.placeholder')}
        >
          <Search className="h-5 w-5" />
        </Button>

        {/* Account menu — folds the previously always-visible controls. */}
        <div className="relative" ref={menuRef}>
          <button
            type="button"
            onClick={() => setMenuOpen((v) => !v)}
            className="flex items-center gap-1.5 h-8 pl-1 pr-1.5 rounded-full hover:bg-accent transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/50"
            aria-label={t('header.account')}
            aria-expanded={menuOpen}
          >
            <span
              className="grid place-items-center h-7 w-7 rounded-full text-brand-foreground"
              style={{ backgroundImage: 'var(--gradient-brand)' }}
            >
              <User className="h-4 w-4" />
            </span>
            <ChevronDown className="h-3.5 w-3.5 text-muted-foreground" aria-hidden="true" />
          </button>

          {menuOpen && (
            <div className="absolute right-0 top-[calc(100%+6px)] w-56 rounded-xl border border-border bg-card p-1.5 shadow-md z-[110]">
              <div className="px-2.5 pt-1.5 pb-1 text-[10px] uppercase tracking-wide text-muted-foreground">
                {t('header.session')}
              </div>
              <button
                type="button"
                className={menuItemClass}
                onClick={() => {
                  setMenuOpen(false);
                  navigate('/quickstart');
                }}
              >
                <Rocket />
                {t('nav.quickstart')}
              </button>
              <div className="px-0.5 py-0.5">
                <ReloadDaemonButton onReloaded={() => window.location.reload()} compact />
              </div>
              <button
                type="button"
                className={menuItemClass}
                onClick={() => {
                  setMenuOpen(false);
                  setSettingsOpen(true);
                }}
              >
                <Settings />
                {t('settings.title')}
              </button>
              <hr className="my-1 border-t border-border" />
              <button
                type="button"
                className="flex w-full items-center gap-2.5 rounded-lg px-2.5 py-2 text-sm text-status-error text-left transition-colors hover:bg-status-error/10 [&_svg]:h-[17px] [&_svg]:w-[17px] [&_svg]:shrink-0"
                onClick={() => {
                  setMenuOpen(false);
                  handleLogout();
                }}
              >
                <LogOut />
                {t('auth.logout')}
              </button>
            </div>
          )}
        </div>
      </header>

      <SettingsModal open={settingsOpen} onClose={() => setSettingsOpen(false)} />
    </>
  );
}
