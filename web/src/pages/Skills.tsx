import { useCallback, useEffect, useState } from 'react';
import { BookOpen, ChevronRight, RefreshCw, Search } from 'lucide-react';

import { getAgentOptions, listAgentSkills } from '@/lib/api';
import type { AgentSkillEntry, DroppedSkillEntry } from '@/lib/api';
import { plural, t } from '@/lib/i18n';
import { Badge, Button, Card, EmptyState, PageHeader, Select } from '@/components/ui';
import { SettingsPageShell, SettingsListBody, SettingsSelectableRow } from '@/components/ui/settings-list';
import { DetailPanel, DetailPanelSurface, DetailSectionTitle } from '@/components/ui/detail-panel';
import { IconTile } from '@/components/ui/icon-tile';
import { SpinnerScreen } from '@/components/ui/spinner';

const skillKey = (s: AgentSkillEntry): string =>
  s.editable && s.bundle ? `${s.bundle}/${s.name}` : `${s.origin}:${s.plugin ?? ''}/${s.name}`;

export default function Skills() {
  const [agents, setAgents] = useState<string[]>([]);
  const [selectedAlias, setSelectedAlias] = useState<string>('');
  const [skills, setSkills] = useState<AgentSkillEntry[]>([]);
  const [dropped, setDropped] = useState<DroppedSkillEntry[]>([]);
  const [search, setSearch] = useState('');
  const [loading, setLoading] = useState(true);
  const [reloading, setReloading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [selectedKey, setSelectedKey] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    getAgentOptions()
      .then(({ agents: as }) => {
        if (cancelled) return;
        setAgents(as);
        setSelectedAlias((prev) => prev || as[0] || '');
        if (as.length === 0) setLoading(false);
      })
      .catch((err: unknown) => {
        if (cancelled) return;
        setError(err instanceof Error ? err.message : String(err));
        setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const loadSkills = useCallback((alias: string) => {
    return listAgentSkills(alias).then(({ skills: ss, dropped: dd }) => {
      setSkills(ss);
      setDropped(dd ?? []);
    });
  }, []);

  useEffect(() => {
    if (!selectedAlias) return;
    setLoading(true);
    setError(null);
    setSelectedKey(null);
    loadSkills(selectedAlias)
      .catch((err: unknown) => setError(err instanceof Error ? err.message : String(err)))
      .finally(() => setLoading(false));
  }, [selectedAlias, loadSkills]);

  const handleReload = () => {
    if (!selectedAlias) return;
    setReloading(true);
    loadSkills(selectedAlias)
      .catch((err: unknown) => setError(err instanceof Error ? err.message : String(err)))
      .finally(() => setReloading(false));
  };

  const filtered = skills.filter((s) => {
    const q = search.toLowerCase();
    return (
      s.name.toLowerCase().includes(q) ||
      s.description.toLowerCase().includes(q) ||
      s.origin.toLowerCase().includes(q) ||
      (s.bundle ?? '').toLowerCase().includes(q) ||
      (s.plugin ?? '').toLowerCase().includes(q)
    );
  });

  const selectedSkill = selectedKey ? skills.find((s) => skillKey(s) === selectedKey) ?? null : null;

  if (error) {
    return (
      <SettingsPageShell>
        <Card padded={false} className="p-4 text-sm border-status-error/25 bg-status-error/10 text-status-error">
          {t('skills.load_error')}: {error}
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
    <div className="flex h-full min-h-0">
      <div className="no-scrollbar min-w-0 flex-1 overflow-y-auto">
        <SettingsPageShell>
          <PageHeader
            title={t('skills.title')}
            actions={
              <Button variant="ghost" onClick={handleReload} disabled={reloading}>
                <RefreshCw className={`h-4 w-4 ${reloading ? 'animate-spin' : ''}`} />
                {t('skills.reload')}
              </Button>
            }
          />

          <div className="flex flex-wrap items-center gap-2">
            {agents.length > 0 && (
              <Select
                value={selectedAlias}
                onChange={setSelectedAlias}
                options={agents.map((a) => ({ value: a, label: a }))}
                aria-label={t('skills.agent')}
                className="w-48"
              />
            )}
            <div className="relative min-w-0 flex-1">
              <Search className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-muted-foreground" />
              <input
                type="text"
                value={search}
                onChange={(e) => setSearch(e.target.value)}
                placeholder={t('skills.search')}
                className="h-9 w-full rounded-lg border border-border bg-input pl-9 pr-3 text-sm text-foreground focus:border-border-strong focus:outline-none"
              />
            </div>
          </div>

          {dropped.length > 0 && (
            <Card
              padded={false}
              className="space-y-1 p-4 text-sm border-status-warning/25 bg-status-warning/10 text-status-warning"
            >
              {plural(dropped.length, 'skills.skipped_count')}
              <ul className="mt-1 space-y-0.5">
                {dropped.map((d) => (
                  <li key={`${d.origin}/${d.name}`} className="text-xs text-muted-foreground">
                    <span className="font-mono">{d.name}</span> ({d.origin}) — {d.reason}
                  </li>
                ))}
              </ul>
            </Card>
          )}

          {filtered.length === 0 && dropped.length === 0 ? (
            <EmptyState icon={<BookOpen className="h-6 w-6" />} title={t('skills.empty')} />
          ) : (
            <SettingsListBody>
              {filtered.map((s) => {
                const key = skillKey(s);
                return (
                  <SettingsSelectableRow
                    key={key}
                    ariaLabel={s.name}
                    isSelected={selectedKey === key}
                    onSelect={() => setSelectedKey(key)}
                    leading={
                      <IconTile>
                        <BookOpen className="h-[18px] w-[18px] text-muted-foreground" />
                      </IconTile>
                    }
                    title={
                      <span className="flex items-center gap-2">
                        <span className="font-mono">{s.name}</span>
                        <Badge tone="neutral">{s.bundle ?? s.plugin ?? s.origin}</Badge>
                      </span>
                    }
                    subtitle={s.description}
                    trailingIcon={<ChevronRight className="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />}
                  />
                );
              })}
            </SettingsListBody>
          )}
        </SettingsPageShell>
      </div>

      <DetailPanelSurface open={selectedSkill !== null}>
        {selectedSkill && (
          <DetailPanel
            icon={
              <IconTile>
                <BookOpen className="h-[18px] w-[18px] text-muted-foreground" />
              </IconTile>
            }
            title={<span className="font-mono">{selectedSkill.name}</span>}
            subtitle={selectedSkill.origin}
            onClose={() => setSelectedKey(null)}
          >
            <div className="flex flex-col gap-5">
              <div>
                <DetailSectionTitle>{t('skills.about')}</DetailSectionTitle>
                <p className="mt-2 whitespace-pre-wrap text-sm text-foreground/90">{selectedSkill.description}</p>
              </div>
              <div>
                <DetailSectionTitle>{t('skills.origin')}</DetailSectionTitle>
                <div className="mt-2 flex flex-wrap gap-1.5">
                  <Badge tone="neutral">{selectedSkill.origin}</Badge>
                  {selectedSkill.bundle && <Badge tone="neutral">{selectedSkill.bundle}</Badge>}
                  {selectedSkill.plugin && <Badge tone="neutral">{selectedSkill.plugin}</Badge>}
                </div>
              </div>
              {selectedSkill.directory && (
                <div>
                  <DetailSectionTitle>{t('skills.location')}</DetailSectionTitle>
                  <p className="mt-2 break-all font-mono text-xs text-muted-foreground">{selectedSkill.directory}</p>
                </div>
              )}
            </div>
          </DetailPanel>
        )}
      </DetailPanelSurface>
    </div>
  );
}
