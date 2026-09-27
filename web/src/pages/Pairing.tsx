import { useState, useEffect, useCallback } from 'react';
import { Smartphone, Trash2, X } from 'lucide-react';
import {
  getAdminPairCode,
  generatePairCode,
  getPrincipals,
  type PrincipalSummary,
} from '@/lib/api';
import { Button, Card, ConfirmDialog, PageHeader, Select } from '@/components/ui';
import { t, fmtDate } from '@/lib/i18n';

/** Sentinel `<Select>` value for "mint an untagged code" — the current
 *  pending-principal behavior. Never a real principal id (ids come from the
 *  config-driven `authz.principals` table, which can't contain an empty
 *  string as a natural key). */
const NO_ROLE = '';

interface Device {
  id: string;
  name: string | null;
  device_type: string | null;
  paired_at: string;
  last_seen: string;
  ip_address: string | null;
}

export default function Pairing() {
  const [devices, setDevices] = useState<Device[]>([]);
  const [loading, setLoading] = useState(true);
  const [pairingCode, setPairingCode] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  // The device queued for revocation; non-null opens the confirm dialog.
  const [pendingRevoke, setPendingRevoke] = useState<Device | null>(null);
  // True when /api/devices returned 401/403 — this browser isn't paired, so
  // the list can't be read (distinct from an empty registry).
  const [unauthorized, setUnauthorized] = useState(false);
  // Configured principals (F4 authz), for the "which role does this device
  // get" picker on pairing-code generation. Best-effort: an empty list just
  // hides the picker (non-admin caller, endpoint predates this daemon build,
  // or genuinely no principals configured yet) and generation falls back to
  // the untagged pending-* behavior — it never blocks the page.
  const [principals, setPrincipals] = useState<PrincipalSummary[]>([]);
  const [selectedPrincipal, setSelectedPrincipal] = useState(NO_ROLE);
  // The principal id the most recently generated code was tagged for (or
  // null for an untagged code), shown next to the pairing code.
  const [taggedFor, setTaggedFor] = useState<string | null>(null);

  const token = localStorage.getItem('zeroclaw_token') || '';

  const fetchDevices = useCallback(async () => {
    try {
      const res = await fetch('/api/devices', {
        headers: { Authorization: `Bearer ${token}` },
      });
      if (res.ok) {
        const data = await res.json();
        setDevices(data.devices || []);
        setUnauthorized(false);
      } else if (res.status === 401 || res.status === 403) {
        // require_pairing is on and this browser isn't paired (no/invalid
        // token), so the gateway rejects the listing. Distinguish this from a
        // genuinely empty registry — otherwise it reads as "0 paired devices"
        // when really we just can't see them.
        setUnauthorized(true);
        setDevices([]);
      } else {
        setError(t('pairing.load_error'));
      }
    } catch (err) {
      setError(t('pairing.load_error'));
    } finally {
      setLoading(false);
    }
  }, [token]);

  // Fetch the current pairing code on mount (if one is active)
  useEffect(() => {
    getAdminPairCode()
      .then((data) => {
        if (data.pairing_code) {
          setPairingCode(data.pairing_code);
        }
      })
      .catch(() => {
        // Admin endpoint not reachable — code will show after clicking "Pair New Device"
      });
  }, []);

  // Load the principal picker's options. Best-effort: any failure (403 for a
  // non-admin caller, 404 on a daemon build that predates this endpoint, or a
  // network error) just leaves the list empty, which hides the selector below.
  useEffect(() => {
    getPrincipals()
      .then(setPrincipals)
      .catch(() => setPrincipals([]));
  }, []);

  useEffect(() => { fetchDevices(); }, [fetchDevices]);

  const handleInitiatePairing = async () => {
    try {
      const principal = selectedPrincipal || undefined;
      const data = await generatePairCode(principal);
      if (data.pairing_code) {
        setPairingCode(data.pairing_code);
        setTaggedFor(principal ?? null);
      } else {
        setError(data.message || t('pairing.generate_error'));
      }
    } catch (err) {
      // PairCodeForbiddenError (non-loopback origin) and any other failure
      // both land on the same generic message here — this page has no CLI
      // fallback UI (unlike the pre-auth pairing screen in App.tsx).
      setError(t('pairing.generate_error'));
    }
  };

  const handleRevokeDevice = async (deviceId: string) => {
    try {
      const res = await fetch(`/api/devices/${deviceId}`, {
        method: 'DELETE',
        headers: { Authorization: `Bearer ${token}` },
      });
      if (res.ok) {
        setDevices(devices.filter(d => d.id !== deviceId));
      } else {
        setError(t('pairing.revoke_error'));
      }
    } catch (err) {
      setError(t('pairing.revoke_error'));
    } finally {
      setPendingRevoke(null);
    }
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="h-8 w-8 border-2 rounded-full animate-spin border-border border-t-pc-accent" />
      </div>
    );
  }

  return (
    <div className="p-6 space-y-6">
      <PageHeader
        title={t('pairing.title')}
        actions={
          <Button onClick={handleInitiatePairing}>
            <Smartphone className="h-4 w-4" />
            {t('pairing.pair_new_device')}
          </Button>
        }
      />

      {principals.length > 0 && (
        <Card className="flex flex-wrap items-center gap-3 p-4">
          <label htmlFor="pairing-role-select" className="text-sm font-medium text-text-secondary">
            {t('pairing.role_label')}
          </label>
          <Select
            id="pairing-role-select"
            aria-label={t('pairing.role_label')}
            className="max-w-xs"
            value={selectedPrincipal}
            onChange={setSelectedPrincipal}
            options={[
              { value: NO_ROLE, label: t('pairing.role_none') },
              ...principals.map((p) => ({
                value: p.id,
                label: p.admin ? `${p.id} ${t('pairing.role_admin_marker')}` : p.id,
              })),
            ]}
          />
        </Card>
      )}

      {error && (
        <Card className="flex items-start gap-2 text-sm border-status-error/25 bg-status-error/10 text-status-error">
          <span className="flex-1">{error}</span>
          <button
            type="button"
            onClick={() => setError(null)}
            className="flex-shrink-0 text-status-error/70 hover:text-status-error transition-colors"
            aria-label={t('pairing.dismiss')}
          >
            <X className="h-4 w-4" />
          </button>
        </Card>
      )}

      {pairingCode && (
        <Card className="p-6 text-center">
          <p className="text-xs uppercase tracking-wider mb-2 text-muted-foreground">
            {t('pairing.pairing_code')}
          </p>
          <div className="text-4xl font-mono font-bold tracking-[0.4em] py-4 text-foreground">
            {pairingCode}
          </div>
          <p className="text-xs text-muted-foreground">{t('pairing.code_hint')}</p>
          {taggedFor && (
            <p className="mt-2 text-xs text-text-secondary">
              {t('pairing.tagged_for', { value: taggedFor })}
            </p>
          )}
        </Card>
      )}

      <Card padded={false} className="overflow-hidden">
        <div className="px-5 py-4 border-b border-border">
          <h3 className="text-sm font-semibold text-foreground">
            {t('pairing.paired_devices')}
            {unauthorized ? '' : ` (${devices.length})`}
          </h3>
        </div>
        {unauthorized ? (
          <div className="p-8 text-center text-sm text-muted-foreground">
            <p className="font-medium text-text-secondary">
              {t('pairing.unpaired_title')}
            </p>
            <p className="mt-1">
              {t('pairing.unpaired_hint')}
            </p>
          </div>
        ) : devices.length === 0 ? (
          <div className="p-8 text-center text-sm text-muted-foreground">
            {t('pairing.no_devices')}
          </div>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-border text-left text-xs uppercase tracking-wider text-muted-foreground">
                  <th className="px-5 py-3 font-medium">{t('pairing.name')}</th>
                  <th className="px-5 py-3 font-medium">{t('pairing.type')}</th>
                  <th className="px-5 py-3 font-medium">{t('pairing.paired')}</th>
                  <th className="px-5 py-3 font-medium">{t('pairing.last_seen')}</th>
                  <th className="px-5 py-3 font-medium">{t('pairing.ip')}</th>
                  <th className="px-5 py-3 font-medium text-right">
                    {t('pairing.actions')}
                  </th>
                </tr>
              </thead>
              <tbody className="divide-y divide-border">
                {devices.map((device) => (
                  <tr
                    key={device.id}
                    className="transition-colors hover:bg-secondary/50"
                  >
                    <td className="px-5 py-3 text-foreground">
                      {device.name || t('pairing.unnamed')}
                    </td>
                    <td className="px-5 py-3 text-text-secondary">
                      {device.device_type || t('pairing.unknown')}
                    </td>
                    <td className="px-5 py-3 text-xs text-muted-foreground">
                      {fmtDate(device.paired_at)}
                    </td>
                    <td className="px-5 py-3 text-xs text-muted-foreground">
                      {fmtDate(device.last_seen, { dateStyle: 'medium', timeStyle: 'medium' })}
                    </td>
                    <td className="px-5 py-3 font-mono text-xs text-text-secondary">
                      {device.ip_address || '-'}
                    </td>
                    <td className="px-5 py-3 text-right">
                      <Button
                        variant="danger"
                        size="sm"
                        onClick={() => setPendingRevoke(device)}
                        aria-label={t('pairing.actions')}
                      >
                        <Trash2 className="h-4 w-4" />
                      </Button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </Card>

      <ConfirmDialog
        open={pendingRevoke !== null}
        danger
        title={t('pairing.revoke_title')}
        message={t('pairing.revoke_message', {
          value: pendingRevoke?.name || t('pairing.this_device'),
        })}
        confirmLabel={t('pairing.revoke')}
        onConfirm={() => {
          if (pendingRevoke) void handleRevokeDevice(pendingRevoke.id);
        }}
        onClose={() => setPendingRevoke(null)}
      />
    </div>
  );
}
