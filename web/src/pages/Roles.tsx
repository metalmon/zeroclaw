import { useMemo, useState } from 'react';
import { Plus, Shield, ShieldCheck, Trash2, X } from 'lucide-react';
import { useRoles } from '@/hooks/useRoles';
import { HttpError, type AuthzProfile, type AuthzPrincipalSummary } from '@/lib/api';
import { Badge, Button, Card, ConfirmDialog, PageHeader, Select } from '@/components/ui';
import { t } from '@/lib/i18n';

// Sentinel written into `allowed_agents` for "every agent" — matches the
// backend contract (`api_authz.rs` / `AuthzConfig::effective_agents`), which
// treats a literal `"*"` entry as a wildcard grant rather than a real alias.
const ALL_AGENTS = '*';

function friendlyError(err: unknown, fallbackKey: string): string {
  if (err instanceof HttpError && err.status === 403) {
    return t('roles.forbidden_error');
  }
  if (err instanceof Error && err.message) return err.message;
  return t(fallbackKey);
}

/** True when a principal is granted admin via ANY of its bound profiles. */
function isPrincipalAdmin(principal: AuthzPrincipalSummary, profiles: AuthzProfile[]): boolean {
  return principal.profiles.some((pid) => profiles.find((p) => p.id === pid)?.admin === true);
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

function ProfileForm({ form, agents, saving, formError, onChange, onSave, onCancel }: ProfileFormProps) {
  const toggleAgent = (agent: string) => {
    const next = new Set(form.allowedAgents);
    if (next.has(agent)) next.delete(agent);
    else next.add(agent);
    onChange({ ...form, allowedAgents: next });
  };

  return (
    <Card className="space-y-4">
      <h3 className="text-sm font-semibold text-pc-text">
        {form.editingId ? t('roles.edit_profile_title') : t('roles.new_profile_title')}
      </h3>

      {formError && (
        <p className="text-xs text-status-error" role="alert">
          {formError}
        </p>
      )}

      <div className="space-y-1">
        <label htmlFor="roles-profile-id" className="text-xs font-medium text-pc-text-secondary">
          {t('roles.profile_id')}
        </label>
        <input
          id="roles-profile-id"
          type="text"
          value={form.id}
          disabled={form.editingId !== null}
          placeholder={t('roles.profile_id_placeholder')}
          onChange={(e) => onChange({ ...form, id: e.target.value })}
          className="w-full max-w-sm rounded-[var(--radius-md)] border border-pc-border bg-pc-base px-3 py-1.5 text-sm text-pc-text disabled:opacity-50"
        />
        {form.editingId !== null && (
          <p className="text-[11px] text-pc-text-muted">{t('roles.profile_id_immutable_hint')}</p>
        )}
      </div>

      <div className="space-y-1.5">
        <span className="text-xs font-medium text-pc-text-secondary">{t('roles.allowed_agents')}</span>
        <label className="flex items-center gap-2 text-sm text-pc-text">
          <input
            type="checkbox"
            checked={form.allAgents}
            onChange={(e) => onChange({ ...form, allAgents: e.target.checked })}
          />
          {t('roles.all_agents')}
        </label>
        {!form.allAgents && (
          agents.length === 0 ? (
            <p className="text-xs text-pc-text-muted">{t('roles.no_agents_configured')}</p>
          ) : (
            <div className="grid grid-cols-2 sm:grid-cols-3 gap-1.5 max-h-40 overflow-y-auto rounded-[var(--radius-md)] border border-pc-border p-2">
              {agents.map((agent) => (
                <label key={agent} className="flex items-center gap-2 text-sm text-pc-text-secondary">
                  <input
                    type="checkbox"
                    checked={form.allowedAgents.has(agent)}
                    onChange={() => toggleAgent(agent)}
                  />
                  <span className="truncate">{agent}</span>
                </label>
              ))}
            </div>
          )
        )}
      </div>

      <label className="flex items-center gap-2 text-sm text-pc-text">
        <input
          type="checkbox"
          checked={form.admin}
          onChange={(e) => onChange({ ...form, admin: e.target.checked })}
        />
        {t('roles.admin_toggle')}
      </label>

      <div className="flex items-center gap-2 pt-1">
        <Button onClick={onSave} disabled={saving}>
          {t('roles.save')}
        </Button>
        <Button variant="ghost" onClick={onCancel} disabled={saving}>
          {t('roles.cancel')}
        </Button>
      </div>
    </Card>
  );
}

// ── Page ──────────────────────────────────────────────────────────────────

export default function Roles() {
  const {
    profiles,
    principals,
    agents,
    loading,
    error,
    createProfile,
    updateProfile,
    deleteProfile,
    bindProfile,
    unbindProfile,
  } = useRoles();

  const [form, setForm] = useState<ProfileFormState | null>(null);
  const [saving, setSaving] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [pendingDelete, setPendingDelete] = useState<AuthzProfile | null>(null);
  const [deleteAffected, setDeleteAffected] = useState<{ profileId: string; principals: string[] } | null>(null);
  const [bindSelection, setBindSelection] = useState<Record<string, string>>({});

  // PENDING first (they need attention), then everyone else, both
  // alphabetical within their bucket — keeps the unassigned-role rows from
  // getting lost at the bottom of a long principal list.
  const sortedPrincipals = useMemo(() => {
    return [...principals].sort((a, b) => {
      const aPending = a.profiles.length === 0;
      const bPending = b.profiles.length === 0;
      if (aPending !== bPending) return aPending ? -1 : 1;
      return a.id.localeCompare(b.id);
    });
  }, [principals]);

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
      if (result.affected_principals.length > 0) {
        setDeleteAffected({ profileId: target.id, principals: result.affected_principals });
      }
    } catch (err) {
      setActionError(friendlyError(err, 'roles.delete_error'));
    }
  };

  const handleBind = async (principalId: string) => {
    const profileId = bindSelection[principalId];
    if (!profileId) return;
    setActionError(null);
    try {
      await bindProfile(principalId, profileId);
      setBindSelection((prev) => ({ ...prev, [principalId]: '' }));
    } catch (err) {
      setActionError(friendlyError(err, 'roles.bind_error'));
    }
  };

  const handleUnbind = async (principalId: string, profileId: string) => {
    setActionError(null);
    try {
      await unbindProfile(principalId, profileId);
    } catch (err) {
      setActionError(friendlyError(err, 'roles.unbind_error'));
    }
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="h-8 w-8 border-2 rounded-full animate-spin border-pc-border border-t-pc-accent" />
      </div>
    );
  }

  return (
    <div className="p-6 space-y-6">
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
        <Card className="flex items-start gap-2 text-sm border-status-error/25 bg-status-error/10 text-status-error">
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
        <Card className="text-sm border-status-warning/25 bg-status-warning/10 text-status-warning space-y-1">
          <div className="flex items-start justify-between gap-2">
            <span>
              {deleteAffected.principals.length} {t('roles.delete_profile_affected')}
            </span>
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

      {form && (
        <ProfileForm
          form={form}
          agents={agents}
          saving={saving}
          formError={formError}
          onChange={setForm}
          onSave={() => void handleSave()}
          onCancel={closeForm}
        />
      )}

      {/* Permission profiles */}
      <Card padded={false} className="overflow-hidden">
        <div className="px-5 py-4 border-b border-pc-border">
          <h3 className="text-sm font-semibold text-pc-text">
            {t('roles.profiles_heading')} ({profiles.length})
          </h3>
        </div>
        {profiles.length === 0 ? (
          <div className="p-8 text-center text-sm text-pc-text-muted">{t('roles.no_profiles')}</div>
        ) : (
          <ul className="divide-y divide-pc-border">
            {profiles.map((profile) => (
              <li key={profile.id} className="flex items-center justify-between gap-3 px-5 py-3">
                <div className="min-w-0 flex items-center gap-2 flex-wrap">
                  {profile.admin ? (
                    <ShieldCheck className="h-4 w-4 shrink-0 text-pc-accent" aria-hidden="true" />
                  ) : (
                    <Shield className="h-4 w-4 shrink-0 text-pc-text-muted" aria-hidden="true" />
                  )}
                  <span className="font-mono text-sm text-pc-text truncate">{profile.id}</span>
                  {profile.admin && <Badge tone="ok">{t('roles.admin_badge')}</Badge>}
                  <Badge tone="neutral">
                    {profile.allowed_agents.includes(ALL_AGENTS)
                      ? t('roles.all_agents')
                      : `${profile.allowed_agents.length}`}
                  </Badge>
                </div>
                <div className="flex items-center gap-2 flex-shrink-0">
                  <Button variant="ghost" size="sm" onClick={() => openEdit(profile)}>
                    {t('roles.edit')}
                  </Button>
                  <Button variant="danger" size="sm" onClick={() => setPendingDelete(profile)}>
                    <Trash2 className="h-4 w-4" />
                  </Button>
                </div>
              </li>
            ))}
          </ul>
        )}
      </Card>

      {/* Principal binding */}
      <Card padded={false} className="overflow-hidden">
        <div className="px-5 py-4 border-b border-pc-border">
          <h3 className="text-sm font-semibold text-pc-text">
            {t('roles.principals_heading')} ({principals.length})
          </h3>
        </div>
        {principals.length === 0 ? (
          <div className="p-8 text-center text-sm text-pc-text-muted">{t('roles.no_principals')}</div>
        ) : (
          <ul className="divide-y divide-pc-border">
            {sortedPrincipals.map((principal) => {
              const pending = principal.profiles.length === 0;
              const unbound = profiles.filter((p) => !principal.profiles.includes(p.id));
              return (
                <li key={principal.id} className="px-5 py-3 space-y-2">
                  <div className="flex items-center gap-2 flex-wrap">
                    <span className="font-mono text-sm text-pc-text">{principal.id}</span>
                    {isPrincipalAdmin(principal, profiles) && <Badge tone="ok">{t('roles.admin_badge')}</Badge>}
                    {pending && <Badge tone="warn">{t('roles.pending_badge')}</Badge>}
                    <span className="text-[11px] text-pc-text-muted">
                      {principal.tokenHashCount} {t('roles.token_count')} · {principal.deviceIdCount}{' '}
                      {t('roles.device_count')}
                    </span>
                  </div>
                  <div className="flex items-center gap-2 flex-wrap">
                    {principal.profiles.map((pid) => (
                      <Badge key={pid} tone="neutral">
                        {pid}
                        <button
                          type="button"
                          onClick={() => void handleUnbind(principal.id, pid)}
                          aria-label={`${t('roles.unbind')} ${pid}`}
                          className="hover:text-status-error"
                        >
                          <X className="h-3 w-3" />
                        </button>
                      </Badge>
                    ))}
                    {unbound.length > 0 && (
                      <div className="flex items-center gap-1.5">
                        <Select
                          value={bindSelection[principal.id] ?? ''}
                          onChange={(v) => setBindSelection((prev) => ({ ...prev, [principal.id]: v }))}
                          options={unbound.map((p) => ({ value: p.id, label: p.id }))}
                          placeholder={t('roles.select_profile_placeholder')}
                          aria-label={t('roles.bind_profile')}
                          className="w-44"
                        />
                        <Button
                          size="sm"
                          variant="ghost"
                          disabled={!bindSelection[principal.id]}
                          onClick={() => void handleBind(principal.id)}
                        >
                          {t('roles.bind_profile')}
                        </Button>
                      </div>
                    )}
                  </div>
                  {principal.legacyAllowedAgents.length > 0 && (
                    <p className="text-[11px] text-pc-text-muted">
                      {t('roles.legacy_agents_hint')} {principal.legacyAllowedAgents.join(', ')}
                    </p>
                  )}
                </li>
              );
            })}
          </ul>
        )}
      </Card>

      <ConfirmDialog
        open={pendingDelete !== null}
        danger
        title={t('roles.delete_profile_title')}
        message={
          <>
            {t('roles.delete_profile_message_prefix')}{' '}
            <span className="font-mono text-pc-text-secondary">{pendingDelete?.id}</span>
            {t('roles.delete_profile_message_suffix')}
          </>
        }
        confirmLabel={t('roles.delete')}
        onConfirm={() => void confirmDelete()}
        onClose={() => setPendingDelete(null)}
      />
    </div>
  );
}
