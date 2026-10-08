import { useEffect, useState, useCallback } from 'react';
import { Link } from 'react-router-dom';
import { Bot, ChevronRight, Plus, Power, AlertCircle } from 'lucide-react';
import AgentDrawer from '@/components/AgentDrawer';
import { Badge, Button, Card, EmptyState, PageHeader } from '@/components/ui';
import { SettingsPageShell, SettingsListBody, SettingsSelectableRow } from '@/components/ui/settings-list';
import { IconTile } from '@/components/ui/icon-tile';
import { SpinnerScreen } from '@/components/ui/spinner';
import { t } from '@/lib/i18n';
import { loadAgentSummaries, toggleAgentEnabled, type AgentSummary } from '@/lib/agents';

interface AgentSummariesState {
  loading: boolean;
  error: string | null;
  agents: AgentSummary[];
}

export default function AgentsList() {
  const [state, setState] = useState<AgentSummariesState>({ loading: true, error: null, agents: [] });
  const [toggling, setToggling] = useState<Set<string>>(new Set());
  const [selectedAlias, setSelectedAlias] = useState<string | null>(null);

  const refresh = useCallback(() => {
    setState((s) => ({ ...s, loading: true, error: null }));
    loadAgentSummaries()
      .then((agents) => setState({ loading: false, error: null, agents }))
      .catch((err: unknown) =>
        setState({
          loading: false,
          error: err instanceof Error ? err.message : t('agents_list.load_failed'),
          agents: [],
        }),
      );
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const toggleEnabled = useCallback(async (agent: AgentSummary) => {
    setToggling((prev) => new Set(prev).add(agent.alias));
    try {
      await toggleAgentEnabled(agent.alias, !agent.enabled);
      setState((s) => ({
        ...s,
        agents: s.agents.map((a) => (a.alias === agent.alias ? { ...a, enabled: !a.enabled } : a)),
      }));
    } catch (err) {
      setState((s) => ({
        ...s,
        error: err instanceof Error ? err.message : `${t('agents_list.toggle_failed_prefix')}${agent.alias}`,
      }));
    } finally {
      setToggling((prev) => {
        const next = new Set(prev);
        next.delete(agent.alias);
        return next;
      });
    }
  }, []);

  const selectedAgent =
    selectedAlias === null ? null : state.agents.find((a) => a.alias === selectedAlias) ?? null;

  return (
    <div className="flex h-full min-h-0">
      <div className="no-scrollbar min-w-0 flex-1 overflow-y-auto">
        <SettingsPageShell>
        <PageHeader
          title={t('nav.agents')}
          description={t('agents_list.description')}
          actions={
            <Link to="/config/agents">
              <Button variant="default" size="default">
                <Plus className="h-4 w-4" />
                {t('agents_list.new_agent')}
              </Button>
            </Link>
          }
        />

        {state.error && (
          <Card
            padded={false}
            className="flex items-start gap-2 p-4 text-sm border-status-error/25 bg-status-error/10 text-status-error"
          >
            <AlertCircle className="mt-0.5 h-4 w-4 flex-shrink-0" />
            <span>{state.error}</span>
          </Card>
        )}

        {state.loading && state.agents.length === 0 ? (
          <SpinnerScreen />
        ) : state.agents.length === 0 ? (
          <AgentsEmpty />
        ) : (
          <SettingsListBody>
            {state.agents.map((agent) => (
              <SettingsSelectableRow
                key={agent.alias}
                ariaLabel={agent.alias}
                isSelected={agent.alias === selectedAlias}
                onSelect={() => setSelectedAlias(agent.alias)}
                leading={
                  <IconTile>
                    <Bot className="h-[18px] w-[18px] text-muted-foreground" />
                  </IconTile>
                }
                title={
                  <span className="flex items-center gap-2">
                    <span className="font-medium">{agent.displayName || agent.alias}</span>
                    {agent.displayName ? (
                      <span className="text-xs font-mono text-muted-foreground">{agent.alias}</span>
                    ) : null}
                    <Badge tone={agent.enabled ? 'ok' : 'neutral'}>
                      <Power className="h-3 w-3" />
                      {agent.enabled ? t('agent.enabled') : t('agent.disabled')}
                    </Badge>
                  </span>
                }
                subtitle={agent.modelProvider || t('agent.no_model_provider')}
                trailingIcon={<ChevronRight className="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />}
              />
            ))}
          </SettingsListBody>
        )}
        </SettingsPageShell>
      </div>

      <AgentDrawer
        agent={selectedAgent}
        onClose={() => setSelectedAlias(null)}
        onToggle={toggleEnabled}
        toggling={selectedAgent ? toggling.has(selectedAgent.alias) : false}
      />
    </div>
  );
}

function AgentsEmpty() {
  return (
    <EmptyState
      icon={<Bot className="h-6 w-6" />}
      title={t('agents_list.empty_title')}
      hint={t('agents_list.empty_hint')}
      action={
        <Link to="/quickstart" className="inline-block">
          <Button variant="default" size="default">
            <Plus className="h-4 w-4" />
            {t('agents_list.start_quickstart')}
          </Button>
        </Link>
      }
    />
  );
}
