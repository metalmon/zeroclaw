import { useLocation } from 'react-router-dom';
import { basePath } from '../../lib/basePath';
import { findActiveNavPath } from './sidebarNav';
import { SidebarNavLink } from './SidebarNavLink';
import {
  Activity,
  ArrowDownToLine,
  Bot,
  Clock,
  LayoutDashboard,
  ListChecks,
  MessageSquare,
  Monitor,
  Puzzle,
  Settings,
  ShieldCheck,
  Smartphone,
  Sparkles,
  Stethoscope,
  Terminal,
  Workflow,
  Wrench,
} from 'lucide-react';
import { t } from '@/lib/i18n';
import { useEffect, useState } from 'react';
import { getStatus } from '@/lib/api';
import { useVersionCheck } from '@/hooks/useVersionCheck';
import { UpgradeDialog } from '@/components/UpgradeDialog';
import type { StatusResponse } from '@/types/api';

interface NavItem {
  to: string;
  icon: typeof LayoutDashboard;
  labelKey: string;
}

interface NavGroup {
  headingKey: string;
  items: NavItem[];
}

// Grouped navigation. Every existing route/link is preserved — the flat list
// is just organized under four clusters so the rail reads top-down by task:
// Home → Chat → Configure → Operations. On the desktop rail the cluster
// boundaries become thin divider rules (no text headings); the mobile drawer
// still renders the headings as full labels.
const navGroups: NavGroup[] = [
  {
    headingKey: 'nav.group.home',
    items: [{ to: '/', icon: LayoutDashboard, labelKey: 'nav.dashboard' }],
  },
  {
    headingKey: 'nav.group.chat',
    items: [{ to: '/agents', icon: MessageSquare, labelKey: 'nav.agents' }],
  },
  {
    headingKey: 'nav.group.configure',
    items: [
      { to: '/config', icon: Settings, labelKey: 'nav.config' },
      { to: '/config/agents', icon: Bot, labelKey: 'nav.agent' },
      { to: '/tools', icon: Wrench, labelKey: 'nav.tools' },
      { to: '/skills', icon: Sparkles, labelKey: 'nav.skills' },
      { to: '/sops', icon: Workflow, labelKey: 'nav.sops' },
      { to: '/runs', icon: ListChecks, labelKey: 'nav.runs' },
      { to: '/integrations', icon: Puzzle, labelKey: 'nav.integrations' },
      { to: '/cron', icon: Clock, labelKey: 'nav.cron' },
    ],
  },
  {
    headingKey: 'nav.group.operations',
    items: [
      { to: '/logs', icon: Activity, labelKey: 'nav.logs' },
      { to: '/pairing', icon: Smartphone, labelKey: 'nav.pairing' },
      { to: '/roles', icon: ShieldCheck, labelKey: 'nav.roles' },
      { to: '/doctor', icon: Stethoscope, labelKey: 'nav.doctor' },
      { to: '/canvas', icon: Monitor, labelKey: 'nav.canvas' },
      { to: '/acp-console', icon: Terminal, labelKey: 'nav.acp' },
    ],
  },
];

// NavLink matches path prefixes by default. Resolve the longest registered
// destination so a specific item can suppress its otherwise-active ancestors.
const navPaths = navGroups.flatMap((group) => group.items.map((item) => item.to));

// The 6 Quickstart sections (Workspace, Providers, Channels, Memory,
// Hardware, Tunnel) live under /config now — they're the first group
// inside the Config explorer's sidebar. The /setup/<section> deep-link
// route still works for bookmarks, but no top-level nav entries point
// at it. Run-setup-again link in /config covers the wizard re-entry.

// ── Desktop rail item ───────────────────────────────────────────────────────
// Icon-only nav item for the slim rail. The icon is the affordance; the label
// is exposed three ways: title (native tooltip, used as a screen-reader /
// no-JS fallback), aria-label (screen readers), and a token-styled popover to
// the right shown on hover OR keyboard focus.
//
// The popover is rendered into `document.body` via `createPortal` so it
// lives outside the desktop `<nav>`'s DOM subtree (the `<nav>` is a scroll
// container with `overflow-y: auto`; rendering the popover inside the
// subtree would make it a positioned descendant of that scroll container,
// which we don't want). With the portal, the popover is rendered into
// the top-level body and `position: fixed` pins it to viewport coords
// computed from the NavLink's bounding rect, with `scroll` / `resize`
// listeners re-anchoring it while it is visible. Because the popover was
// the only content that ever extended past the rail's right border, the
// nav needs no horizontal overflow handling at all: `overflow-y: auto`
// alone leaves `scrollWidth === clientWidth`, so no `overflow-x` value
// (hidden or clip) is applied to either the `<nav>` or the `<aside>`.
// ── Mobile drawer item ──────────────────────────────────────────────────────
// Full labelled row (icon + text) for the mobile drawer, with the same calm
// active treatment as before: subtle accent tint, 2px left accent bar, accent
// icon, and aria-current via NavLink.
function DrawerNavItem({
  item,
  activePath,
  onClick,
}: {
  item: NavItem;
  activePath: string | null;
  onClick: () => void;
}) {
  const { to, icon: Icon, labelKey } = item;
  const text = t(labelKey);
  return (
    <SidebarNavLink
      to={to}
      activePath={activePath}
      onClick={onClick}
      className={({ isActive }) =>
        [
          'group relative flex items-center gap-3 p-2',
          'rounded-xl text-sm transition-colors duration-150',
          isActive
            ? 'bg-sidebar-accent text-sidebar-accent-foreground font-medium'
            : 'text-sidebar-foreground hover:bg-sidebar-accent/60 hover:text-sidebar-accent-foreground',
        ].join(' ')
      }
    >
      {({ isActive }) => (
        <>
          <Icon
            className={`h-[18px] w-[18px] shrink-0 transition-colors ${
              isActive ? '' : 'text-muted-foreground group-hover:text-sidebar-accent-foreground'
            }`}
          />
          <span className="whitespace-nowrap">{text}</span>
        </>
      )}
    </SidebarNavLink>
  );
}

// ── Mobile drawer group ─────────────────────────────────────────────────────
// One labelled cluster: a faint uppercase heading associated with its <ul> via
// aria-labelledby so screen readers announce the group name.
function DrawerGroup({ group, index, activePath, onClick }: {
  group: NavGroup;
  index: number;
  activePath: string | null;
  onClick: () => void;
}) {
  const heading = t(group.headingKey);
  const headingId = `nav-group-${index}`;
  return (
    <div role="group" aria-labelledby={headingId} className="space-y-0.5">
      <h2
        id={headingId}
        className="px-3 pt-3 pb-1 text-[10px] font-semibold uppercase tracking-wider select-none"
        style={{ color: 'var(--color-text-faint)' }}
      >
        {heading}
      </h2>
      {group.items.map((item) => (
        <DrawerNavItem
          key={item.to}
          item={item}
          activePath={activePath}
          onClick={onClick}
        />
      ))}
    </div>
  );
}

interface SidebarProps {
  open: boolean;
  onClose: () => void;
}

export default function Sidebar({ open, onClose }: SidebarProps) {
  const { pathname } = useLocation();
  const activePath = findActiveNavPath(pathname, navPaths);
  const [status, setStatus] = useState<StatusResponse | null>(null);
  useEffect(() => {
    getStatus()
      .then(setStatus)
      .catch(() => { /* silently ignore */ });
  }, []);
  // `check_updates` is undefined on older gateways → treat as enabled.
  const checkUpdates = status?.check_updates !== false;
  const { info, loading, refetch } = useVersionCheck(checkUpdates);
  const hasUpdate = info?.is_newer === true;
  const version = status?.version ?? null;
  const [upgradeOpen, setUpgradeOpen] = useState(false);
  const openUpgrade = () => setUpgradeOpen(true);

  return (
    <>
      {/* Backdrop — mobile only */}
      {open && (
        <div
          className="md:hidden fixed inset-0 z-40 bg-black/60 backdrop-blur-sm transition-opacity"
          onClick={onClose}
          onKeyDown={(e) => { if (e.key === 'Escape') onClose(); }}
          role="button"
          tabIndex={-1}
          aria-label={t('sidebar.close_menu')}
        />
      )}

      {/* Desktop rail — permanent slim icon rail, always 56px. No collapse
          toggle: the rail is the navigation. Grouping is expressed as thin
          divider rules between the icon clusters. */}
      {/* Desktop sidebar — persistent labelled column (Thunderbolt style):
          seamless (same ground as content, no divider), rows are labelled with
          the active item on a light tinted pill. Brand lives in the title bar. */}
      <aside
        className="hidden md:flex fixed top-0 left-0 h-screen w-60 flex-col bg-sidebar border-r border-sidebar-border z-50"
        aria-label={t('nav.aria.primary')}
      >
        <nav className="no-scrollbar flex-1 overflow-y-auto pt-3 pb-2 px-2 space-y-0.5" aria-label={t('nav.aria.primary')}>
          {navGroups.map((group, index) => (
            <DrawerGroup
              key={group.headingKey}
              group={group}
              index={index}
              activePath={activePath}
              onClick={onClose}
            />
          ))}
        </nav>
        <DrawerFooter version={version} hasUpdate={hasUpdate} onOpen={openUpgrade} />
      </aside>

      {/* Mobile drawer — labelled full version (icons + labels), slides in/out. */}
      <aside
        className={[
          'md:hidden fixed top-0 left-0 h-screen w-60 flex flex-col border-r z-50 transition-transform duration-200 ease-out',
          open ? 'translate-x-0' : '-translate-x-full',
        ].join(' ')}
        style={{ background: 'var(--color-sidebar)', borderColor: 'var(--color-border)' }}
        aria-label={t('sidebar.mobile_menu')}
      >
        <DrawerLogo />
        <nav className="no-scrollbar flex-1 overflow-y-auto py-3 px-2 space-y-0.5" aria-label={t('nav.aria.primary')}>
          {navGroups.map((group, index) => (
            <DrawerGroup
              key={group.headingKey}
              group={group}
              index={index}
              activePath={activePath}
              onClick={onClose}
            />
          ))}
        </nav>
        <DrawerFooter version={version} hasUpdate={hasUpdate} onOpen={openUpgrade} />
      </aside>

      <UpgradeDialog
        open={upgradeOpen}
        info={info}
        loading={loading}
        checkUpdatesEnabled={checkUpdates}
        allowSelfUpgrade={status?.allow_self_upgrade === true}
        restartMode={status?.restart_mode}
        restartHint={status?.restart_hint}
        onRefetch={refetch}
        onClose={() => setUpgradeOpen(false)}
      />
    </>
  );
}

// ── Logo / mark ─────────────────────────────────────────────────────────────

// Compact mark for the slim rail — the logo image only, centered, no wordmark.
// Full mark + wordmark for the mobile drawer.
function DrawerLogo() {
  return (
    <div
      className="flex items-center border-b shrink-0 overflow-hidden"
      style={{ borderColor: 'var(--color-border)', height: '56px', padding: '0 16px', gap: '12px' }}
    >
      <div className="relative shrink-0">
        <div
          className="absolute -inset-1.5 rounded-xl"
          style={{ background: 'linear-gradient(135deg, rgba(var(--color-accent-rgb), 0.15), rgba(var(--color-accent-rgb), 0.05))' }}
        />
        <img
          src={`${basePath}/_app/logo.png`}
          alt={t('sidebar.logo_alt')}
          className="relative h-9 w-9 rounded-xl object-cover"
          onError={(e) => {
            e.currentTarget.style.display = 'none';
          }}
        />
      </div>
      <span
        className="text-sm font-semibold tracking-wide whitespace-nowrap"
        style={{ color: 'var(--color-foreground)' }}
      >
        {t('sidebar.brand')}
      </span>
    </div>
  );
}

// ── Footers ─────────────────────────────────────────────────────────────────

interface FooterProps {
  version: string | null;
  /** A newer release is available — render an accent dot. */
  hasUpdate: boolean;
  /** Open the upgrade dialog. */
  onOpen: () => void;
}

// Rail footer — version tag as a button, centered, with a native tooltip
// carrying the full "Volt Gateway vX" string since the rail has no room for
// the label. When an update is available the version row is replaced by a
// pulsing download-arrow icon stacked above the version text — the dot was
// too easy to miss against the muted `text-faint` colour.
// Drawer footer — full labelled gateway line for mobile, clickable to upgrade.
// When an update is available the dot is replaced by a soft-bouncing download
// arrow rendered at body-text size so the affordance is unmistakable.
function DrawerFooter({ version, hasUpdate, onOpen }: FooterProps) {
  return (
    <div
      className="px-5 py-2.5 border-t text-[10px] uppercase tracking-wider"
      style={{ borderColor: 'var(--color-border)', color: 'var(--color-text-faint)' }}
    >
      <button
        type="button"
        onClick={onOpen}
        title={hasUpdate ? t('sidebar.update_available') : undefined}
        className={[
          'flex items-center gap-1.5 cursor-pointer transition-opacity uppercase tracking-wider',
          hasUpdate ? 'opacity-100' : 'hover:opacity-80',
        ].join(' ')}
        style={hasUpdate ? { color: 'var(--color-primary)' } : undefined}
      >
        {hasUpdate && (
          <ArrowDownToLine
            aria-hidden="true"
            className="h-3.5 w-3.5 animate-bounce-soft"
          />
        )}
        <span>
          {hasUpdate ? t('sidebar.update_available') : t('sidebar.gateway')}
        </span>
      </button>
      {version && (
        <div className="mt-0.5 normal-case tracking-normal" style={{ fontSize: '9px' }}>
          v{version}
        </div>
      )}
    </div>
  );
}
