import { useEffect, useMemo, useState } from 'react';
import { ChevronRight, Plus, Trash2, User, UserRound, X } from 'lucide-react';
import { useRoles } from '@/hooks/useRoles';
import {
  HttpError,
  forgetExternalSubject,
  getMapKeys,
  type AuthzProfile,
  type AuthzPrincipalSummary,
  type ExternalSubject,
} from '@/lib/api';
import { Badge, Button, Card, ConfirmDialog, EmptyState, PageHeader, Select } from '@/components/ui';
import {
  SettingsPageShell,
  SettingsListBody,
  SettingsSectionLabel,
  SettingsSelectableRow,
} from '@/components/ui/settings-list';
import { DetailPanel, DetailPanelSurface, DetailSectionTitle } from '@/components/ui/detail-panel';
import { IconTile } from '@/components/ui/icon-tile';
import { ActionMenu } from '@/components/ui/action-menu';
import { SpinnerScreen } from '@/components/ui/spinner';
import { formatRelative } from '@/lib/format';
import { plural, t } from '@/lib/i18n';
import { friendlyError } from './Roles';

/** True when a user is granted admin via ANY of its bound roles. */
function isPrincipalAdmin(principal: AuthzPrincipalSummary, profiles: AuthzProfile[]): boolean {
  return principal.profiles.some((pid) => profiles.find((p) => p.id === pid)?.admin === true);
}

/** User ids are config map keys and CLI/URL arguments: keep them to a
 *  conservative token alphabet so they never need quoting or escaping. */
const USER_ID_RE = /^[A-Za-z0-9._@-]+$/;

// ── Local user detail (bind / unbind roles) ─────────────────────────────────

function PrincipalDetail({
  principal,
  profiles,
  bindValue,
  onBindValueChange,
  onBind,
  onUnbind,
}: {
  principal: AuthzPrincipalSummary;
  profiles: AuthzProfile[];
  bindValue: string;
  onBindValueChange: (value: string) => void;
  onBind: () => void;
  onUnbind: (profileId: string) => void;
}) {
  const unbound = profiles.filter((p) => !principal.profiles.includes(p.id));
  return (
    <div className="flex flex-col gap-5">
      <div>
        <DetailSectionTitle>{t('users.roles')}</DetailSectionTitle>
        <div className="mt-2 flex flex-wrap items-center gap-1.5">
          {principal.profiles.length === 0 && (
            <span className="text-sm text-muted-foreground">{t('users.no_roles_bound')}</span>
          )}
          {principal.profiles.map((pid) => (
            <Badge key={pid} tone="neutral">
              {pid}
              <button
                type="button"
                onClick={() => onUnbind(pid)}
                aria-label={`${t('users.unbind')} ${pid}`}
                className="hover:text-status-error"
              >
                <X className="h-3 w-3" />
              </button>
            </Badge>
          ))}
        </div>
      </div>

      {unbound.length > 0 && (
        <div>
          <DetailSectionTitle>{t('users.bind_profile')}</DetailSectionTitle>
          <div className="mt-2 flex items-center gap-2">
            <Select
              value={bindValue}
              onChange={onBindValueChange}
              options={unbound.map((p) => ({ value: p.id, label: p.id }))}
              placeholder={t('users.select_profile_placeholder')}
              aria-label={t('users.bind_profile')}
              className="flex-1"
            />
            <Button size="sm" disabled={!bindValue} onClick={onBind}>
              {t('users.bind_profile')}
            </Button>
          </div>
        </div>
      )}

      {principal.legacyAllowedAgents.length > 0 && (
        <p className="text-[11px] text-muted-foreground">
          {t('users.legacy_agents_hint')} {principal.legacyAllowedAgents.join(', ')}
        </p>
      )}
    </div>
  );
}

// ── Local user create form (pairing-code login) ─────────────────────────────

interface UserFormState {
  id: string;
  profiles: Set<string>;
}

function UserForm({
  form,
  profiles,
  hasIdp,
  saving,
  formError,
  onChange,
  onSave,
  onCancel,
}: {
  form: UserFormState;
  profiles: AuthzProfile[];
  /** An `[oidc.*]` alias is configured: remind that SSO users need no manual entry. */
  hasIdp: boolean;
  saving: boolean;
  formError: string | null;
  onChange: (next: UserFormState) => void;
  onSave: () => void;
  onCancel: () => void;
}) {
  const toggleProfile = (id: string) => {
    const next = new Set(form.profiles);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    onChange({ ...form, profiles: next });
  };

  return (
    <div className="flex flex-col gap-5">
      {formError && (
        <p className="text-xs text-status-error" role="alert">
          {formError}
        </p>
      )}

      {hasIdp && (
        <Card padded={false} className="p-3 text-xs border-status-warning/25 bg-status-warning/10 text-status-warning">
          {t('users.idp_warning')}
        </Card>
      )}

      <div className="space-y-1">
        <label htmlFor="users-local-id" className="text-xs font-medium text-text-secondary">
          {t('users.id')}
        </label>
        <input
          id="users-local-id"
          type="text"
          value={form.id}
          autoComplete="off"
          spellCheck={false}
          placeholder={t('users.id_placeholder')}
          onChange={(e) => onChange({ ...form, id: e.target.value })}
          className="w-full rounded-lg border border-border bg-input px-3 py-2 text-sm text-foreground focus:border-border-strong focus:outline-none"
        />
        <p className="text-[11px] text-muted-foreground">{t('users.id_hint')}</p>
      </div>

      <div className="space-y-1.5">
        <span className="text-xs font-medium text-text-secondary">{t('users.roles')}</span>
        {profiles.length === 0 ? (
          <p className="text-xs text-muted-foreground">{t('roles.no_profiles')}</p>
        ) : (
          <div className="grid grid-cols-2 gap-1.5 max-h-48 overflow-y-auto rounded-lg border border-border p-2">
            {profiles.map((profile) => (
              <label key={profile.id} className="flex items-center gap-2 text-sm text-text-secondary">
                <input
                  type="checkbox"
                  checked={form.profiles.has(profile.id)}
                  onChange={() => toggleProfile(profile.id)}
                />
                <span className="truncate font-mono">{profile.id}</span>
                {profile.admin && <Badge tone="ok">{t('roles.admin_badge')}</Badge>}
              </label>
            ))}
          </div>
        )}
      </div>

      <div className="flex items-center justify-end gap-2 pt-2">
        <Button variant="ghost" onClick={onCancel} disabled={saving}>
          {t('roles.cancel')}
        </Button>
        <Button onClick={onSave} disabled={saving}>
          {t('users.new_local')}
        </Button>
      </div>
    </div>
  );
}

// ── Page ──────────────────────────────────────────────────────────────────

export default function Users() {
  const {
    profiles,
    principals,
    external,
    loading,
    error,
    refetch,
    bindProfile,
    unbindProfile,
    createPrincipal,
    deletePrincipal,
  } = useRoles();

  const [actionError, setActionError] = useState<string | null>(null);
  const [bindSelection, setBindSelection] = useState<Record<string, string>>({});
  const [editPrincipalId, setEditPrincipalId] = useState<string | null>(null);
  const [form, setForm] = useState<UserFormState | null>(null);
  const [saving, setSaving] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);
  const [pendingDelete, setPendingDelete] = useState<AuthzPrincipalSummary | null>(null);
  /** Set when a plain delete came back 409: devices/tokens are still bound. */
  const [forceDelete, setForceDelete] = useState<AuthzPrincipalSummary | null>(null);
  const [hasIdp, setHasIdp] = useState(false);
  const [pendingForget, setPendingForget] = useState<ExternalSubject | null>(null);

  useEffect(() => {
    getMapKeys('oidc')
      .then(({ keys }) => setHasIdp(keys.length > 0))
      .catch(() => setHasIdp(false));
  }, []);

  // PENDING first (they need attention), then everyone else, both
  // alphabetical within their bucket.
  const sortedPrincipals = useMemo(() => {
    return [...principals].sort((a, b) => {
      const aPending = a.profiles.length === 0;
      const bPending = b.profiles.length === 0;
      if (aPending !== bPending) return aPending ? -1 : 1;
      return a.id.localeCompare(b.id);
    });
  }, [principals]);

  const editPrincipal = editPrincipalId ? principals.find((p) => p.id === editPrincipalId) ?? null : null;

  const openPrincipal = (id: string) => {
    setForm(null);
    setEditPrincipalId(id);
  };
  const openCreate = () => {
    setEditPrincipalId(null);
    setFormError(null);
    setForm({ id: '', profiles: new Set() });
  };
  const closeDetail = () => {
    setForm(null);
    setFormError(null);
    setEditPrincipalId(null);
  };

  const handleBind = async (principalId: string) => {
    const profileId = bindSelection[principalId];
    if (!profileId) return;
    setActionError(null);
    try {
      await bindProfile(principalId, profileId);
      setBindSelection((prev) => ({ ...prev, [principalId]: '' }));
    } catch (err) {
      setActionError(friendlyError(err, 'users.bind_error'));
    }
  };

  const handleUnbind = async (principalId: string, profileId: string) => {
    setActionError(null);
    try {
      await unbindProfile(principalId, profileId);
    } catch (err) {
      setActionError(friendlyError(err, 'users.unbind_error'));
    }
  };

  const handleCreate = async () => {
    if (!form) return;
    const id = form.id.trim();
    if (!id) {
      setFormError(t('users.id_required'));
      return;
    }
    if (!USER_ID_RE.test(id)) {
      setFormError(t('users.id_invalid'));
      return;
    }
    if (principals.some((p) => p.id === id)) {
      setFormError(t('users.id_taken'));
      return;
    }
    setSaving(true);
    setFormError(null);
    try {
      await createPrincipal(id, [...form.profiles]);
      setForm(null);
      setEditPrincipalId(id);
    } catch (err) {
      if (err instanceof HttpError && err.status === 409) {
        setFormError(t('users.id_taken'));
      } else {
        setFormError(friendlyError(err, 'users.create_error'));
      }
    } finally {
      setSaving(false);
    }
  };

  const confirmDelete = async (force: boolean) => {
    const target = force ? forceDelete : pendingDelete;
    setPendingDelete(null);
    setForceDelete(null);
    if (!target) return;
    setActionError(null);
    try {
      await deletePrincipal(target.id, force);
      if (editPrincipalId === target.id) closeDetail();
    } catch (err) {
      if (!force && err instanceof HttpError && err.status === 409) {
        // Devices/tokens are still paired to it: ask again, this time with force.
        setForceDelete(target);
        return;
      }
      setActionError(friendlyError(err, 'users.delete_error'));
    }
  };

  const confirmForget = async () => {
    const target = pendingForget;
    setPendingForget(null);
    if (!target) return;
    setActionError(null);
    try {
      await forgetExternalSubject(target.id);
      await refetch();
    } catch (err) {
      setActionError(friendlyError(err, 'users.forget_error'));
    }
  };

  if (loading) {
    return <SpinnerScreen />;
  }

  const externalLabel = (s: ExternalSubject) => s.display_name || s.email || s.subject;

  return (
    <div className="flex h-full min-h-0">
      <div className="no-scrollbar min-w-0 flex-1 overflow-y-auto">
        <SettingsPageShell>
          <PageHeader
            title={t('users.title')}
            description={t('users.description')}
            actions={
              <Button onClick={openCreate}>
                <Plus className="h-4 w-4" />
                {t('users.new_local')}
              </Button>
            }
          />

          {(error || actionError) && (
            <Card
              padded={false}
              className="flex items-start gap-2 p-4 text-sm border-status-error/25 bg-status-error/10 text-status-error"
            >
              <span className="flex-1">{error ? t('users.load_error') : actionError}</span>
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

          {/* Local users (pairing-code login) */}
          <SettingsSectionLabel>
            {t('users.local_heading')} · {principals.length}
          </SettingsSectionLabel>
          {principals.length === 0 ? (
            <EmptyState icon={<User className="h-6 w-6" />} title={t('users.no_local')} />
          ) : (
            <SettingsListBody>
              {sortedPrincipals.map((principal) => {
                const pending = principal.profiles.length === 0;
                return (
                  <SettingsSelectableRow
                    key={principal.id}
                    ariaLabel={principal.id}
                    isSelected={editPrincipalId === principal.id}
                    onSelect={() => openPrincipal(principal.id)}
                    leading={
                      <IconTile>
                        <User className="h-[18px] w-[18px] text-muted-foreground" />
                      </IconTile>
                    }
                    title={
                      <span className="flex items-center gap-2">
                        <span className="font-mono">{principal.id}</span>
                        {isPrincipalAdmin(principal, profiles) && <Badge tone="ok">{t('roles.admin_badge')}</Badge>}
                        {pending && <Badge tone="warn">{t('users.pending_badge')}</Badge>}
                      </span>
                    }
                    subtitle={`${plural(principal.tokenHashCount, 'users.token_count')} · ${plural(
                      principal.deviceIdCount,
                      'users.device_count',
                    )}`}
                    trailingIcon={<ChevronRight className="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />}
                  />
                );
              })}
            </SettingsListBody>
          )}

          {/* External (SSO) users — read-only roster, hidden on gateways without the route */}
          {external !== null && (
            <>
              <SettingsSectionLabel>
                {t('users.external_heading')} · {external.length}
              </SettingsSectionLabel>
              {external.length === 0 ? (
                <EmptyState icon={<UserRound className="h-6 w-6" />} title={t('users.no_external')} />
              ) : (
                <SettingsListBody>
                  {external.map((subject) => (
                    <SettingsSelectableRow
                      key={subject.id}
                      ariaLabel={externalLabel(subject)}
                      leading={
                        <IconTile>
                          <UserRound className="h-[18px] w-[18px] text-muted-foreground" />
                        </IconTile>
                      }
                      title={
                        <span className="flex flex-wrap items-center gap-2">
                          <span>{externalLabel(subject)}</span>
                          <Badge tone="neutral">{subject.provider}</Badge>
                          {subject.admin && <Badge tone="ok">{t('roles.admin_badge')}</Badge>}
                          {subject.profiles.map((pid) => (
                            <Badge key={pid} tone="neutral">
                              {pid}
                            </Badge>
                          ))}
                        </span>
                      }
                      subtitle={
                        subject.last_seen
                          ? t('users.last_login', { value: formatRelative(subject.last_seen) })
                          : t('users.never_logged_in')
                      }
                      trailing={
                        <Button size="sm" variant="ghost" onClick={() => setPendingForget(subject)}>
                          {t('users.forget')}
                        </Button>
                      }
                    />
                  ))}
                </SettingsListBody>
              )}
              <p className="px-0.5 text-xs text-muted-foreground">{t('users.external_hint')}</p>
            </>
          )}
        </SettingsPageShell>
      </div>

      {/* Right detail column — create a local user, or manage one. */}
      <DetailPanelSurface open={form !== null || editPrincipal !== null}>
        {form && (
          <DetailPanel
            icon={
              <IconTile>
                <User className="h-[18px] w-[18px] text-muted-foreground" />
              </IconTile>
            }
            title={t('users.new_local_title')}
            subtitle={t('users.local_heading')}
            onClose={closeDetail}
          >
            <UserForm
              form={form}
              profiles={profiles}
              hasIdp={hasIdp}
              saving={saving}
              formError={formError}
              onChange={setForm}
              onSave={() => void handleCreate()}
              onCancel={closeDetail}
            />
          </DetailPanel>
        )}
        {editPrincipal && (
          <DetailPanel
            icon={
              <IconTile>
                <User className="h-[18px] w-[18px] text-muted-foreground" />
              </IconTile>
            }
            title={<span className="font-mono">{editPrincipal.id}</span>}
            subtitle={`${plural(editPrincipal.tokenHashCount, 'users.token_count')} · ${plural(
              editPrincipal.deviceIdCount,
              'users.device_count',
            )}`}
            onClose={closeDetail}
            actions={
              <ActionMenu
                items={[
                  {
                    label: t('roles.delete'),
                    icon: <Trash2 />,
                    danger: true,
                    onClick: () => setPendingDelete(editPrincipal),
                  },
                ]}
              />
            }
          >
            <PrincipalDetail
              principal={editPrincipal}
              profiles={profiles}
              bindValue={bindSelection[editPrincipal.id] ?? ''}
              onBindValueChange={(v) => setBindSelection((prev) => ({ ...prev, [editPrincipal.id]: v }))}
              onBind={() => void handleBind(editPrincipal.id)}
              onUnbind={(pid) => void handleUnbind(editPrincipal.id, pid)}
            />
          </DetailPanel>
        )}
      </DetailPanelSurface>

      <ConfirmDialog
        open={pendingDelete !== null}
        danger
        title={t('users.delete_title')}
        message={t('users.delete_message', { value: pendingDelete?.id ?? '' })}
        confirmLabel={t('roles.delete')}
        onConfirm={() => void confirmDelete(false)}
        onClose={() => setPendingDelete(null)}
      />

      <ConfirmDialog
        open={forceDelete !== null}
        danger
        title={t('users.delete_bound_title')}
        message={t('users.delete_bound_message', { value: forceDelete?.id ?? '' })}
        confirmLabel={t('users.delete_force')}
        onConfirm={() => void confirmDelete(true)}
        onClose={() => setForceDelete(null)}
      />

      <ConfirmDialog
        open={pendingForget !== null}
        danger
        title={t('users.forget_title')}
        message={t('users.forget_message', { value: pendingForget ? externalLabel(pendingForget) : '' })}
        confirmLabel={t('users.forget')}
        onConfirm={() => void confirmForget()}
        onClose={() => setPendingForget(null)}
      />
    </div>
  );
}
