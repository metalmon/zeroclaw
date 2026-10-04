import { useMemo, useState } from 'react';
import { ChevronRight, Plus, Shield, ShieldCheck, Trash2, X } from 'lucide-react';
import { useRoles } from '@/hooks/useRoles';
import { HttpError, type AuthzProfile } from '@/lib/api';
import { Badge, Button, Card, ConfirmDialog, EmptyState, PageHeader } from '@/components/ui';
import {
  SettingsPageShell,
  SettingsListBody,
  SettingsSectionLabel,
  SettingsSelectableRow,
} from '@/components/ui/settings-list';
import { DetailPanel, DetailPanelSurface } from '@/components/ui/detail-panel';
import { IconTile } from '@/components/ui/icon-tile';
import { ActionMenu } from '@/components/ui/action-menu';
import { SpinnerScreen } from '@/components/ui/spinner';
import { plural, t } from '@/lib/i18n';

// Sentinel written into `allowed_agents` for "every agent" — matches the
// backend contract (`api_authz.rs` / `AuthzConfig::effective_agents`), which
// treats a literal `"*"` entry as a wildcard grant rather than a real alias.
const ALL_AGENTS = '*';

export function friendlyError(err: unknown, fallbackKey: string): string {
  if (err instanceof HttpError && err.status === 403) {
    return t('roles.forbidden_error');
  }
  if (err instanceof Error && err.message) return err.message;
  return t(fallbackKey);
}

// ── Profile create/edit form ─────────────────────────────────────────────

interface ProfileFormState {
  /** Non-null while editing an existing profile (id is then read-only). */
  editingId: string | null;
  id: string;
  allowedAgents: Set<string>;
  allAgents: boolean;
  admin: boolean;
}

function emptyForm(): ProfileFormState {
  return { editingId: null, id: '', allowedAgents: new Set(), allAgents: false, admin: false };
}

function formFromProfile(profile: AuthzProfile): ProfileFormState {
  const allAgents = profile.allowed_agents.includes(ALL_AGENTS);
  return {
    editingId: profile.id,
    id: profile.id,
    allowedAgents: new Set(allAgents ? [] : profile.allowed_agents),
    allAgents,
    admin: profile.admin,
  };
}

interface ProfileFormProps {
  form: ProfileFormState;
  agents: string[];
  saving: boolean;
  formError: string | null;
  onChange: (next: ProfileFormState) => void;
  onSave: () => void;
  onCancel: () => void;
}

// Fields only — the framing (title, close) is provided by the DetailPanel that
// hosts this form in the right drawer.
function ProfileForm({ form, agents, saving, formError, onChange, onSave, onCancel }: ProfileFormProps) {
  const toggleAgent = (agent: string) => {
    const next = new Set(form.allowedAgents);
    if (next.has(agent)) next.delete(agent);
    else next.add(agent);
    onChange({ ...form, allowedAgents: next });
  };

  return (
    <div className="flex flex-col gap-5">
      {formError && (
        <p className="text-xs text-status-error" role="alert">
          {formError}
        </p>
      )}

      <div className="space-y-1">
        <label htmlFor="roles-profile-id" className="text-xs font-medium text-text-secondary">
          {t('roles.profile_id')}
        </label>
        <input
          id="roles-profile-id"
          type="text"
          value={form.id}
          disabled={form.editingId !== null}
          placeholder={t('roles.profile_id_placeholder')}
          onChange={(e) => onChange({ ...form, id: e.target.value })}
          className="w-full rounded-lg border border-border bg-input px-3 py-2 text-sm text-foreground disabled:opacity-50 focus:border-border-strong focus:outline-none"
        />
        {form.editingId !== null && (
          <p className="text-[11px] text-muted-foreground">{t('roles.profile_id_immutable_hint')}</p>
        )}
      </div>

      <div className="space-y-1.5">
        <span className="text-xs font-medium text-text-secondary">{t('roles.allowed_agents')}</span>
        <label className="flex items-center gap-2 text-sm text-foreground">
          <input
            type="checkbox"
            checked={form.allAgents}
            onChange={(e) => onChange({ ...form, allAgents: e.target.checked })}
          />
          {t('roles.all_agents')}
        </label>
        {!form.allAgents &&
          (agents.length === 0 ? (
            <p className="text-xs text-muted-foreground">{t('roles.no_agents_configured')}</p>
          ) : (
            <div className="grid grid-cols-2 gap-1.5 max-h-48 overflow-y-auto rounded-lg border border-border p-2">
              {agents.map((agent) => (
                <label key={agent} className="flex items-center gap-2 text-sm text-text-secondary">
                  <input
                    type="checkbox"
                    checked={form.allowedAgents.has(agent)}
                    onChange={() => toggleAgent(agent)}
                  />
                  <span className="truncate">{agent}</span>
                </label>
              ))}
            </div>
          ))}
      </div>

      <label className="flex items-center gap-2 text-sm text-foreground">
        <input
          type="checkbox"
          checked={form.admin}
          onChange={(e) => onChange({ ...form, admin: e.target.checked })}
        />
        {t('roles.admin_toggle')}
      </label>

      <div className="flex items-center justify-end gap-2 pt-2">
        <Button variant="ghost" onClick={onCancel} disabled={saving}>
          {t('roles.cancel')}
        </Button>
        <Button onClick={onSave} disabled={saving}>
          {t('roles.save')}
        </Button>
      </div>
    </div>
  );
}

// ── Page ──────────────────────────────────────────────────────────────────

export default function Roles() {
  const { profiles, principals, external, agents, loading, error, createProfile, updateProfile, deleteProfile } =
    useRoles();

  const [form, setForm] = useState<ProfileFormState | null>(null);
  const [saving, setSaving] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [pendingDelete, setPendingDelete] = useState<AuthzProfile | null>(null);
  const [deleteAffected, setDeleteAffected] = useState<{ profileId: string; principals: string[] } | null>(null);

  // How many users hold each role, local and external (SSO) separately — a
  // count, not a management surface (users are bound/unbound on the Users page).
  const userCounts = useMemo(() => {
    const counts = new Map<string, { local: number; external: number }>();
    const bump = (pid: string, kind: 'local' | 'external') => {
      const c = counts.get(pid) ?? { local: 0, external: 0 };
      c[kind] += 1;
      counts.set(pid, c);
    };
    for (const principal of principals) for (const pid of principal.profiles) bump(pid, 'local');
    for (const subject of external ?? []) for (const pid of subject.profiles) bump(pid, 'external');
    return counts;
  }, [principals, external]);

  const userCountLabel = (profileId: string): string => {
    const c = userCounts.get(profileId);
    if (!c || (c.local === 0 && c.external === 0)) return plural(0, 'roles.user_count');
    return `${plural(c.local, 'roles.local_count')} · ${plural(c.external, 'roles.external_count')}`;
  };

  const openCreate = () => {
    setFormError(null);
    setForm(emptyForm());
  };
  const openEdit = (profile: AuthzProfile) => {
    setFormError(null);
    setForm(formFromProfile(profile));
  };
  const closeForm = () => {
    setForm(null);
    setFormError(null);
  };

  const handleSave = async () => {
    if (!form) return;
    const id = form.id.trim();
    if (!id) {
      setFormError(t('roles.id_required'));
      return;
    }
    if (form.editingId === null && profiles.some((p) => p.id === id)) {
      setFormError(t('roles.id_taken'));
      return;
    }
    const allowed_agents = form.allAgents ? [ALL_AGENTS] : [...form.allowedAgents];
    setSaving(true);
    setFormError(null);
    try {
      if (form.editingId === null) {
        await createProfile({ id, allowed_agents, admin: form.admin });
      } else {
        await updateProfile({ id, allowed_agents, admin: form.admin });
      }
      setForm(null);
    } catch (err) {
      setFormError(friendlyError(err, 'roles.save_error'));
    } finally {
      setSaving(false);
    }
  };

  const confirmDelete = async () => {
    if (!pendingDelete) return;
    const target = pendingDelete;
    setPendingDelete(null);
    setActionError(null);
    try {
      const result = await deleteProfile(target.id);
      if (form?.editingId === target.id) closeForm();
      if (result.affected_principals.length > 0) {
        setDeleteAffected({ profileId: target.id, principals: result.affected_principals });
      }
    } catch (err) {
      setActionError(friendlyError(err, 'roles.delete_error'));
    }
  };

  if (loading) {
    return <SpinnerScreen />;
  }

  return (
    <div className="flex h-full min-h-0">
      <div className="no-scrollbar min-w-0 flex-1 overflow-y-auto">
        <SettingsPageShell>
          <PageHeader
            title={t('roles.title')}
            description={t('roles.description')}
            actions={
              <Button onClick={openCreate}>
                <Plus className="h-4 w-4" />
                {t('roles.new_profile')}
              </Button>
            }
          />

          {(error || actionError) && (
            <Card
              padded={false}
              className="flex items-start gap-2 p-4 text-sm border-status-error/25 bg-status-error/10 text-status-error"
            >
              <span className="flex-1">{error ? t('roles.load_error') : actionError}</span>
              <button
                type="button"
                onClick={() => setActionError(null)}
                className="flex-shrink-0 text-status-error/70 hover:text-status-error transition-colors"
                aria-label={t('roles.dismiss')}
              >
                <X className="h-4 w-4" />
              </button>
            </Card>
          )}

          {deleteAffected && (
            <Card
              padded={false}
              className="space-y-1 p-4 text-sm border-status-warning/25 bg-status-warning/10 text-status-warning"
            >
              <div className="flex items-start justify-between gap-2">
                <span>{plural(deleteAffected.principals.length, 'roles.delete_profile_affected')}</span>
                <button
                  type="button"
                  onClick={() => setDeleteAffected(null)}
                  className="flex-shrink-0 text-status-warning/70 hover:text-status-warning transition-colors"
                  aria-label={t('roles.dismiss')}
                >
                  <X className="h-4 w-4" />
                </button>
              </div>
              <p className="font-mono text-xs">{deleteAffected.principals.join(', ')}</p>
            </Card>
          )}

          <SettingsSectionLabel>
            {t('roles.profiles_heading')} · {profiles.length}
          </SettingsSectionLabel>
          {profiles.length === 0 ? (
            <EmptyState icon={<Shield className="h-6 w-6" />} title={t('roles.no_profiles')} />
          ) : (
            <SettingsListBody>
              {profiles.map((profile) => (
                <SettingsSelectableRow
                  key={profile.id}
                  ariaLabel={profile.id}
                  isSelected={form?.editingId === profile.id}
                  onSelect={() => openEdit(profile)}
                  leading={
                    <IconTile className={profile.admin ? 'bg-status-success/10 text-status-success' : undefined}>
                      {profile.admin ? (
                        <ShieldCheck className="h-[18px] w-[18px]" />
                      ) : (
                        <Shield className="h-[18px] w-[18px]" />
                      )}
                    </IconTile>
                  }
                  title={
                    <span className="flex items-center gap-2">
                      <span className="font-mono">{profile.id}</span>
                      {profile.admin && <Badge tone="ok">{t('roles.admin_badge')}</Badge>}
                      <Badge tone="neutral">
                        {profile.allowed_agents.includes(ALL_AGENTS)
                          ? t('roles.all_agents')
                          : `${profile.allowed_agents.length}`}
                      </Badge>
                    </span>
                  }
                  subtitle={userCountLabel(profile.id)}
                  trailingIcon={<ChevronRight className="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />}
                />
              ))}
            </SettingsListBody>
          )}
        </SettingsPageShell>
      </div>

      {/* Right detail column — create / edit a role. */}
      <DetailPanelSurface open={form !== null}>
        {form && (
          <DetailPanel
            icon={
              <IconTile className="text-brand-foreground [background-image:var(--gradient-brand)]">
                <ShieldCheck className="h-[18px] w-[18px]" />
              </IconTile>
            }
            title={<span className="font-mono">{form.editingId ?? t('roles.new_profile_title')}</span>}
            subtitle={form.editingId ? t('nav.roles') : undefined}
            onClose={closeForm}
            actions={
              form.editingId ? (
                <ActionMenu
                  items={[
                    {
                      label: t('roles.delete'),
                      icon: <Trash2 />,
                      danger: true,
                      onClick: () => {
                        const p = profiles.find((pr) => pr.id === form.editingId);
                        if (p) setPendingDelete(p);
                      },
                    },
                  ]}
                />
              ) : undefined
            }
          >
            <ProfileForm
              form={form}
              agents={agents}
              saving={saving}
              formError={formError}
              onChange={setForm}
              onSave={() => void handleSave()}
              onCancel={closeForm}
            />
          </DetailPanel>
        )}
      </DetailPanelSurface>

      <ConfirmDialog
        open={pendingDelete !== null}
        danger
        title={t('roles.delete_profile_title')}
        message={t('roles.delete_profile_message', { value: pendingDelete?.id ?? '' })}
        confirmLabel={t('roles.delete')}
        onConfirm={() => void confirmDelete()}
        onClose={() => setPendingDelete(null)}
      />
    </div>
  );
}
