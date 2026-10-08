import { useState, useEffect, useCallback } from 'react';
import {
  bindPrincipalProfile,
  createAuthzPrincipal,
  createAuthzProfile,
  deleteAuthzPrincipal,
  deleteAuthzProfile,
  getAuthzAgents,
  getExternalSubjects,
  loadAuthzPrincipals,
  listAuthzProfiles,
  unbindPrincipalProfile,
  updateAuthzProfile,
  type AuthzProfile,
  type AuthzProfileBody,
  type AuthzPrincipalSummary,
  type DeleteAuthzProfileResponse,
  type ExternalSubject,
} from '@/lib/api';

export interface UseRolesResult {
  profiles: AuthzProfile[];
  principals: AuthzPrincipalSummary[];
  /** External (SSO) users; `null` when the gateway has no such route (404). */
  external: ExternalSubject[] | null;
  /** Every configured agent alias, for the `allowed_agents` multiselect. */
  agents: string[];
  loading: boolean;
  error: string | null;
  refetch: () => Promise<void>;
  createProfile: (body: AuthzProfileBody) => Promise<AuthzProfile>;
  updateProfile: (body: AuthzProfileBody) => Promise<AuthzProfile>;
  /** Resolves with the affected-principal ids so the caller can surface a
   *  "N principals now have a dangling reference" follow-up. */
  deleteProfile: (id: string) => Promise<DeleteAuthzProfileResponse>;
  bindProfile: (principalId: string, profileId: string) => Promise<void>;
  unbindProfile: (principalId: string, profileId: string) => Promise<void>;
  createPrincipal: (id: string, profiles: string[]) => Promise<void>;
  /** `force` drops devices/tokens still bound to the principal (the 409 path). */
  deletePrincipal: (id: string, force?: boolean) => Promise<void>;
}

/**
 * Data hook for the Roles admin screen: permission profiles (CRUD), the
 * principals bound to them, and the agent-alias picker source. Mirrors
 * `useDevices.ts`'s shape (state + loading + error + refetch) but fans out
 * to three endpoints instead of one, and layers mutation helpers that
 * re-fetch on success so the caller never has to hand-roll optimistic-update
 * bookkeeping for what is an infrequently-used admin surface.
 */
export function useRoles(): UseRolesResult {
  const [profiles, setProfiles] = useState<AuthzProfile[]>([]);
  const [principals, setPrincipals] = useState<AuthzPrincipalSummary[]>([]);
  const [external, setExternal] = useState<ExternalSubject[] | null>(null);
  const [agents, setAgents] = useState<string[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const fetchAll = useCallback(async () => {
    try {
      setLoading(true);
      const [profilesResp, principalsList, agentsResp, externalList] = await Promise.all([
        listAuthzProfiles(),
        loadAuthzPrincipals(),
        getAuthzAgents(),
        // Older gateways have no external-users route: that is a hidden group, not an error.
        getExternalSubjects().catch(() => null),
      ]);
      setProfiles(profilesResp.profiles);
      setPrincipals(principalsList);
      setExternal(externalList);
      setAgents(agentsResp.agents);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Unknown error');
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void fetchAll();
  }, [fetchAll]);

  const createProfile = useCallback(
    async (body: AuthzProfileBody) => {
      const created = await createAuthzProfile(body);
      await fetchAll();
      return created;
    },
    [fetchAll],
  );

  const updateProfile = useCallback(
    async (body: AuthzProfileBody) => {
      const updated = await updateAuthzProfile(body);
      await fetchAll();
      return updated;
    },
    [fetchAll],
  );

  const deleteProfile = useCallback(
    async (id: string) => {
      const result = await deleteAuthzProfile(id);
      await fetchAll();
      return result;
    },
    [fetchAll],
  );

  const bindProfile = useCallback(
    async (principalId: string, profileId: string) => {
      await bindPrincipalProfile(principalId, profileId);
      await fetchAll();
    },
    [fetchAll],
  );

  const unbindProfile = useCallback(
    async (principalId: string, profileId: string) => {
      await unbindPrincipalProfile(principalId, profileId);
      await fetchAll();
    },
    [fetchAll],
  );

  const createPrincipal = useCallback(
    async (id: string, profiles: string[]) => {
      await createAuthzPrincipal({ id, profiles });
      await fetchAll();
    },
    [fetchAll],
  );

  const deletePrincipal = useCallback(
    async (id: string, force = false) => {
      await deleteAuthzPrincipal(id, force);
      await fetchAll();
    },
    [fetchAll],
  );

  return {
    profiles,
    principals,
    external,
    agents,
    loading,
    error,
    refetch: fetchAll,
    createProfile,
    updateProfile,
    deleteProfile,
    bindProfile,
    unbindProfile,
    createPrincipal,
    deletePrincipal,
  };
}
