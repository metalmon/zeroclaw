import type { ReactNode, ComponentType } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import {
  BookOpen,
  Bot,
  Brain,
  Clock,
  Database,
  DollarSign,
  MessageSquare,
  Pencil,
  Plug,
  Power,
  Shield,
  Sparkles,
  Users,
  Wifi,
  Zap,
} from 'lucide-react';
import type { LucideProps } from 'lucide-react';
import type { AgentSummary } from '@/lib/agents';
import { Badge } from '@/components/ui';
import { DetailPanel, DetailPanelSurface } from '@/components/ui/detail-panel';
import { IconTile } from '@/components/ui/icon-tile';
import { ActionMenu } from '@/components/ui/action-menu';
import { t } from '@/lib/i18n';
import { formatRelative, formatUsd } from '@/lib/format';
import EntityLink from './EntityLink';

export interface AgentDrawerProps {
  /** The agent to show. When null the panel is closed (renders nothing). */
  agent: AgentSummary | null;
  /** Clear the selection / close the panel. */
  onClose: () => void;
  /** Flip the agent's enabled flag (same handler the list rows use). */
  onToggle: (agent: AgentSummary) => void;
  /** Whether this agent's toggle request is in flight. */
  toggling: boolean;
}

// Calm chip mirroring the list-row treatment: a muted token surface that links
// into config.
const CHIP_CLASS =
  'inline-block font-mono text-[10px] px-2 py-0.5 rounded-full ' +
  'bg-secondary text-text-secondary hover:text-foreground transition-colors';

// A labelled group: a muted caption + icon over a wrapped set of facts. Reused
// for each config dimension so the panel reads as scannable sections.
function DetailGroup({
  icon: Icon,
  label,
  children,
}: {
  icon: ComponentType<LucideProps>;
  label: string;
  children: ReactNode;
}) {
  return (
    <div className="flex flex-col gap-1.5">
      <span className="flex items-center gap-1.5 text-[11px] uppercase tracking-wide text-text-faint">
        <Icon className="h-3 w-3 flex-shrink-0" />
        {label}
      </span>
      <div className="flex flex-wrap items-center gap-1.5 text-sm text-text-secondary">
        {children}
      </div>
    </div>
  );
}

export default function AgentDrawer({
  agent,
  onClose,
  onToggle,
  toggling,
}: AgentDrawerProps) {
  const navigate = useNavigate();
  const open = agent !== null;

  return (
    <DetailPanelSurface open={open}>
      {agent && (
        <DetailPanel
          icon={
            <IconTile>
              <Bot className="h-[18px] w-[18px] text-muted-foreground" />
            </IconTile>
          }
          title={
            <EntityLink
              kind="agent"
              id={agent.alias}
              className="hover:underline"
              title={t('agent.open_config', { value: `agents.${agent.alias}` })}
            >
              {agent.alias}
            </EntityLink>
          }
          subtitle={agent.modelProvider || t('agent.no_model_provider')}
          onClose={onClose}
          actions={
            <ActionMenu
              items={[
                {
                  label: t('agent.open_chat'),
                  icon: <MessageSquare />,
                  onClick: () => navigate(`/agent/${encodeURIComponent(agent.alias)}`),
                },
                {
                  label: t('agent.edit'),
                  icon: <Pencil />,
                  onClick: () => navigate(`/config/agents/${encodeURIComponent(agent.alias)}`),
                },
              ]}
            />
          }
        >
          {/* Status */}
          <div className="flex items-center justify-between gap-3">
            <span className="text-[11px] uppercase tracking-wide text-text-faint">
              {t('common.status')}
            </span>
            <button
              type="button"
              onClick={() => onToggle(agent)}
              disabled={toggling}
              className="rounded-full transition-opacity disabled:opacity-50 focus-visible:outline-none "
              aria-pressed={agent.enabled}
              aria-label={agent.enabled ? t('agent.disable') : t('agent.enable')}
              title={agent.enabled ? t('agent.disable') : t('agent.enable')}
            >
              <Badge tone={agent.enabled ? 'ok' : 'neutral'}>
                <Power className="h-3 w-3" />
                {agent.enabled ? t('agent.enabled') : t('agent.disabled')}
              </Badge>
            </button>
          </div>

          {/* Configuration facts */}
          <DetailGroup icon={Wifi} label={t('agent.section.channels')}>
            {agent.channels.length === 0 ? (
              <span className="text-muted-foreground">{t('agent.none_bound')}</span>
            ) : (
              agent.channels.map((ch) => (
                <EntityLink
                  key={ch}
                  kind="channel"
                  id={ch}
                  className={CHIP_CLASS}
                  title={t('agent.open_config', { value: `channels.${ch}` })}
                >
                  {ch}
                </EntityLink>
              ))
            )}
          </DetailGroup>

          <DetailGroup icon={Shield} label={t('agent.section.profile')}>
            {agent.riskProfile ? (
              <EntityLink
                kind="risk-profile"
                id={agent.riskProfile}
                className="inline-flex items-center gap-1 hover:text-foreground hover:underline"
                title={t('agent.risk_profile_title')}
              >
                {agent.riskProfile}
              </EntityLink>
            ) : (
              <span className="text-muted-foreground" title={t('agent.risk_profile_title')}>
                {t('agent.no_risk_profile')}
              </span>
            )}
            <span className="text-text-faint">·</span>
            <EntityLink
              kind="memory-backend"
              id=""
              className="inline-flex items-center gap-1 hover:text-foreground hover:underline"
              title={
                agent.memoryBackend
                  ? t('agent.memory_backend_title', { value: agent.memoryBackend })
                  : t('agent.memory_backend_default_title')
              }
            >
              <Database className="h-3 w-3 flex-shrink-0" />
              {agent.memoryBackend || t('agent.memory_backend_default')}
            </EntityLink>
            {agent.runtimeProfile && (
              <>
                <span className="text-text-faint">·</span>
                <EntityLink
                  kind="runtime-profile"
                  id={agent.runtimeProfile}
                  className="inline-flex items-center gap-1 hover:text-foreground hover:underline"
                  title={t('agent.runtime_profile_title')}
                >
                  <Zap className="h-3 w-3 flex-shrink-0" />
                  {agent.runtimeProfile}
                </EntityLink>
              </>
            )}
          </DetailGroup>

          {agent.skillBundles.length > 0 && (
            <DetailGroup icon={Sparkles} label={t('agent.section.skills')}>
              {agent.skillBundles.map((s) => (
                <EntityLink
                  key={s}
                  kind="skill-bundle"
                  id={s}
                  className={CHIP_CLASS}
                  title={t('agent.open_config', { value: `skill-bundles.${s}` })}
                >
                  {s}
                </EntityLink>
              ))}
            </DetailGroup>
          )}

          {agent.knowledgeBundles.length > 0 && (
            <DetailGroup icon={BookOpen} label={t('agent.section.knowledge')}>
              {agent.knowledgeBundles.map((k) => (
                <EntityLink
                  key={k}
                  kind="knowledge-bundle"
                  id={k}
                  className={CHIP_CLASS}
                  title={t('agent.open_config', { value: `knowledge-bundles.${k}` })}
                >
                  {k}
                </EntityLink>
              ))}
            </DetailGroup>
          )}

          {agent.mcpBundles.length > 0 && (
            <DetailGroup icon={Plug} label={t('agent.section.mcp')}>
              {agent.mcpBundles.map((m) => (
                <EntityLink
                  key={m}
                  kind="mcp-bundle"
                  id={m}
                  className={CHIP_CLASS}
                  title={t('agent.open_config', { value: `mcp-bundles.${m}` })}
                >
                  {m}
                </EntityLink>
              ))}
            </DetailGroup>
          )}

          {agent.peerGroups.length > 0 && (
            <DetailGroup icon={Users} label={t('agent.section.peers')}>
              {agent.peerGroups.map((pg) => (
                <EntityLink
                  key={pg}
                  kind="peer-group"
                  id={pg}
                  className={CHIP_CLASS}
                  title={t('agent.open_config', { value: `peer_groups.${pg}` })}
                >
                  {pg}
                </EntityLink>
              ))}
            </DetailGroup>
          )}

          {agent.cronJobs.length > 0 && (
            <DetailGroup icon={Clock} label={t('agent.section.cron')}>
              {agent.cronJobs.map((c) => (
                <EntityLink
                  key={c}
                  kind="cron"
                  id={c}
                  className={CHIP_CLASS}
                  title={t('agent.open_config', { value: `cron.${c}` })}
                >
                  {c}
                </EntityLink>
              ))}
            </DetailGroup>
          )}

          {/* Activity stats: sessions / memories / spend */}
          <div className="grid grid-cols-3 gap-2 border-t border-border/60 pt-4">
            <div className="min-w-0">
              <div className="flex items-center gap-1 text-[11px] text-text-faint">
                <MessageSquare className="h-3 w-3 flex-shrink-0" />
                {t('agent.stat.sessions')}
              </div>
              <div className="mt-0.5 text-sm text-foreground">
                {agent.sessionCount === 0 ? (
                  <span className="text-muted-foreground">{t('agent.stat.none')}</span>
                ) : (
                  <Link
                    to={`/?tab=sessions&agent=${encodeURIComponent(agent.alias)}`}
                    className="hover:text-primary hover:underline"
                    title={t('agent.show_sessions_title', { value: agent.alias })}
                  >
                    {agent.sessionCount}
                  </Link>
                )}
              </div>
              <div className="truncate text-[11px] text-muted-foreground">
                {formatRelative(agent.lastActivity)}
              </div>
            </div>

            <div className="min-w-0">
              <div className="flex items-center gap-1 text-[11px] text-text-faint">
                <Brain className="h-3 w-3 flex-shrink-0" />
                {t('agent.stat.memories')}
              </div>
              <div className="mt-0.5 text-sm text-foreground">
                {agent.memoryCount === 0 ? (
                  <span className="text-muted-foreground">{t('agent.stat.none')}</span>
                ) : (
                  <Link
                    to={`/?tab=memories&agent=${encodeURIComponent(agent.alias)}`}
                    className="hover:text-primary hover:underline"
                    title={t('agent.show_memories_title', { value: agent.alias })}
                  >
                    {agent.memoryCount}
                  </Link>
                )}
              </div>
            </div>

            <div
              className="min-w-0"
              title={
                agent.monthCostUsd === null
                  ? t('agent.cost_untracked_title')
                  : t('agent.cost_tracked_title')
              }
            >
              <div className="flex items-center gap-1 text-[11px] text-text-faint">
                <DollarSign className="h-3 w-3 flex-shrink-0" />
                {t('agent.stat.this_month')}
              </div>
              <div className="mt-0.5 text-sm text-foreground">
                {formatUsd(agent.monthCostUsd)}
              </div>
            </div>
          </div>
        </DetailPanel>
      )}
    </DetailPanelSurface>
  );
}
