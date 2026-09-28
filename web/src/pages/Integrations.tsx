import { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { Puzzle, Check, Zap, ChevronRight } from 'lucide-react';
import type { Integration } from '@/types/api';
import { getIntegrations } from '@/lib/api';
import { isChannelAllowed } from '@/lib/channelAllowlist';
import { isProviderAllowed } from '@/lib/providerAllowlist';
import { t } from '@/lib/i18n';
import { Badge, Card, EmptyState, PageHeader } from '@/components/ui';
import type { BadgeTone } from '@/components/ui';
import {
  SettingsPageShell,
  SettingsListBody,
  SettingsSectionLabel,
  SettingsSelectableRow,
} from '@/components/ui/settings-list';
import { IconTile } from '@/components/ui/icon-tile';
import { SpinnerScreen } from '@/components/ui/spinner';

function channelSlug(name: string): string | null {
  const slug = name
    .toLowerCase()
    .trim()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');
  return slug.length > 0 ? slug : null;
}

// Canonical channel key from an integration's display name (underscores, to
// match the channel keys used by VITE_VOLT_CHANNELS: "NextCloud Talk" ->
// "nextcloud_talk"). Differs from channelSlug (hyphens) on purpose.
function channelKeyFromName(name: string): string {
  return name
    .toLowerCase()
    .trim()
    .replace(/[^a-z0-9]+/g, '_')
    .replace(/^_+|_+$/g, '');
}

// Volt build trims the catalog to the RU-relevant set: Chat channels follow the
// channel allowlist (mirrors the add-channel picker); AiModel providers follow
// the provider allowlist. Tools/Platform are left as-is. All passthrough when
// the respective env keep-list is unset.
function isIntegrationVisible(i: Integration): boolean {
  if (i.category === 'Chat') return isChannelAllowed(channelKeyFromName(i.name));
  if (i.category === 'AiModel') return isProviderAllowed(i.name);
  return true;
}

const TOOLS_AUTOMATION_ROUTES: Record<string, string> = {
  cron: '/cron',
  browser: '/config/browser',
  'google workspace': '/config/google_workspace',
};

function configHref(name: string, category: string): string | null {
  const c = category.toLowerCase();
  if (c === 'platform') return null;
  const slug = channelSlug(name);
  if (c.includes('model')) {
    return slug ? `/config/providers.models/${slug}` : '/config/providers.models';
  }
  if (c.includes('chat') || c.includes('channel')) {
    return slug ? `/config/channels/${slug}` : '/config/channels';
  }
  if (c.includes('tool') || c.includes('automation')) {
    return TOOLS_AUTOMATION_ROUTES[name.trim().toLowerCase()] ?? '/tools';
  }
  return '/config';
}

function statusBadge(status: Integration['status']) {
  switch (status) {
    case 'Active':
      return { icon: Check, label: t('integrations.status_active'), tone: 'ok' as BadgeTone };
    case 'Available':
      return { icon: Zap, label: t('integrations.status_available'), tone: 'neutral' as BadgeTone };
    default:
      return { icon: Puzzle, label: String(status), tone: 'neutral' as BadgeTone };
  }
}

const CATEGORY_LABEL_KEYS: Record<string, string> = {
  Chat: 'integrations.cat_chat',
  AiModel: 'integrations.cat_ai_model',
  ToolsAutomation: 'integrations.cat_tools_automation',
  Platform: 'integrations.cat_platform',
};

export default function Integrations() {
  const navigate = useNavigate();
  const [integrations, setIntegrations] = useState<Integration[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [activeCategory, setActiveCategory] = useState<string>('all');

  useEffect(() => {
    getIntegrations()
      .then((list) => setIntegrations(list.filter(isIntegrationVisible)))
      .catch((err) => setError(err.message))
      .finally(() => setLoading(false));
  }, []);

  const categories = ['all', ...Array.from(new Set(integrations.map((i) => i.category))).sort()];
  const filtered =
    activeCategory === 'all'
      ? integrations
      : integrations.filter((i) => i.category === activeCategory);

  const grouped = filtered.reduce<Record<string, Integration[]>>((acc, item) => {
    const key = item.category;
    if (!acc[key]) acc[key] = [];
    acc[key].push(item);
    return acc;
  }, {});

  const labelFor = (cat: string): string => {
    if (cat === 'all') return t('integrations.cat_all');
    const key = CATEGORY_LABEL_KEYS[cat];
    if (key) return t(key);
    return integrations.find((integration) => integration.category === cat)?.category_label ?? cat;
  };

  if (error) {
    return (
      <SettingsPageShell>
        <Card padded={false} className="p-4 text-sm border-status-error/25 bg-status-error/10 text-status-error">
          {t('integrations.load_error')}: {error}
        </Card>
      </SettingsPageShell>
    );
  }

  if (loading) {
    return (
      <SpinnerScreen />
    );
  }

  return (
    <div className="no-scrollbar h-full overflow-y-auto">
      <SettingsPageShell>
        <PageHeader
          title={t('integrations.title')}
          description={t('integrations.subtitle')}
          actions={<Badge tone="neutral">{integrations.length}</Badge>}
        />

        <div className="flex flex-wrap gap-2">
          {categories.map((cat) => {
            const active = activeCategory === cat;
            return (
              <button
                key={cat}
                type="button"
                onClick={() => setActiveCategory(cat)}
                className={`inline-flex h-7 items-center rounded-md border px-3 text-[13px] font-medium transition-colors ${
                  active
                    ? 'border-transparent bg-primary text-primary-foreground'
                    : 'border-border text-muted-foreground hover:bg-accent hover:text-accent-foreground hover:border-border-strong'
                }`}
              >
                {labelFor(cat)}
              </button>
            );
          })}
        </div>

        {Object.keys(grouped).length === 0 ? (
          <EmptyState icon={<Puzzle className="h-6 w-6" />} title={t('integrations.empty')} />
        ) : (
          Object.entries(grouped)
            .sort(([a], [b]) => a.localeCompare(b))
            .map(([category, items]) => (
              <div key={category}>
                <SettingsSectionLabel>{labelFor(category)}</SettingsSectionLabel>
                <SettingsListBody className="mt-2">
                  {items.map((integration) => {
                    const badge = statusBadge(integration.status);
                    const BadgeIcon = badge.icon;
                    const href = configHref(integration.name, integration.category);
                    return (
                      <SettingsSelectableRow
                        key={integration.name}
                        ariaLabel={integration.name}
                        onSelect={href ? () => navigate(href) : undefined}
                        leading={
                          <IconTile>
                            <Puzzle className="h-[18px] w-[18px] text-muted-foreground" />
                          </IconTile>
                        }
                        title={
                          <span className="flex items-center gap-2">
                            <span>{integration.name}</span>
                            <Badge tone={badge.tone}>
                              <BadgeIcon className="h-3 w-3" />
                              {badge.label}
                            </Badge>
                          </span>
                        }
                        subtitle={integration.description}
                        trailingIcon={
                          href ? (
                            <ChevronRight className="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
                          ) : undefined
                        }
                      />
                    );
                  })}
                </SettingsListBody>
              </div>
            ))
        )}
      </SettingsPageShell>
    </div>
  );
}
