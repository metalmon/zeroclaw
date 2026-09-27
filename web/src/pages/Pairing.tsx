import { useState, useEffect, useCallback } from 'react';
import { ChevronRight, Smartphone, Trash2, X } from 'lucide-react';
import {
  getAdminPairCode,
  generatePairCode,
  getPrincipals,
  type PrincipalSummary,
} from '@/lib/api';
import { Button, Card, ConfirmDialog, PageHeader, Select } from '@/components/ui';
import {
  SettingsPageShell,
  SettingsListBody,
  SettingsSectionLabel,
  SettingsSelectableRow,
} from '@/components/ui/settings-list';
import { DetailPanel, DetailPanelSurface, DetailSectionTitle } from '@/components/ui/detail-panel';
import { IconTile } from '@/components/ui/icon-tile';
import { ActionMenu } from '@/components/ui/action-menu';
import { t, fmtDate } from '@/lib/i18n';

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
  const [pendingRevoke, setPendingRevoke] = useState<Device | null>(null);
  const [unauthorized, setUnauthorized] = useState(false);
  const [principals, setPrincipals] = useState<PrincipalSummary[]>([]);
  const [selectedPrincipal, setSelectedPrincipal] = useState(NO_ROLE);
  const [taggedFor, setTaggedFor] = useState<string | null>(null);
  const [selectedDeviceId, setSelectedDeviceId] = useState<string | null>(null);

  const token = localStorage.getItem('zeroclaw_token') || '';

  const fetchDevices = useCallback(async () => {
    try {
      const res = await fetch('/api/devices', { headers: { Authorization: `Bearer ${token}` } });
      if (res.ok) {
        const data = await res.json();
        setDevices(data.devices || []);
        setUnauthorized(false);
      } else if (res.status === 401 || res.status === 403) {
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

  useEffect(() => {
    getAdminPairCode()
      .then((data) => {
        if (data.pairing_code) setPairingCode(data.pairing_code);
      })
      .catch(() => {
        /* admin endpoint not reachable — code shows after Pair New Device */
      });
  }, []);

  useEffect(() => {
    getPrincipals()
      .then(setPrincipals)
      .catch(() => setPrincipals([]));
  }, []);

  useEffect(() => {
    fetchDevices();
  }, [fetchDevices]);

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
        setDevices((prev) => prev.filter((d) => d.id !== deviceId));
        if (selectedDeviceId === deviceId) setSelectedDeviceId(null);
      } else {
        setError(t('pairing.revoke_error'));
      }
    } catch (err) {
      setError(t('pairing.revoke_error'));
    } finally {
      setPendingRevoke(null);
    }
  };

  const selectedDevice = selectedDeviceId ? devices.find((d) => d.id === selectedDeviceId) ?? null : null;

  if (loading) {
    return (
      <div className="flex h-64 items-center justify-center">
        <div className="h-8 w-8 animate-spin rounded-full border-2 border-border border-t-primary" />
      </div>
    );
  }

  return (
    <div className="flex h-full min-h-0">
      <div className="no-scrollbar min-w-0 flex-1 overflow-y-auto">
        <SettingsPageShell>
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
            <Card
              padded={false}
              className="flex items-start gap-2 p-4 text-sm border-status-error/25 bg-status-error/10 text-status-error"
            >
              <span className="flex-1">{error}</span>
              <button
                type="button"
                onClick={() => setError(null)}
                className="flex-shrink-0 text-status-error/70 transition-colors hover:text-status-error"
                aria-label={t('pairing.dismiss')}
              >
                <X className="h-4 w-4" />
              </button>
            </Card>
          )}

          {pairingCode && (
            <Card className="p-6 text-center">
              <p className="mb-2 text-xs uppercase tracking-wider text-muted-foreground">
                {t('pairing.pairing_code')}
              </p>
              <div className="py-4 font-mono text-4xl font-bold tracking-[0.4em] text-foreground">
                {pairingCode}
              </div>
              <p className="text-xs text-muted-foreground">{t('pairing.code_hint')}</p>
              {taggedFor && (
                <p className="mt-2 text-xs text-text-secondary">{t('pairing.tagged_for', { value: taggedFor })}</p>
              )}
            </Card>
          )}

          <SettingsSectionLabel>
            {t('pairing.paired_devices')}
            {unauthorized ? '' : ` · ${devices.length}`}
          </SettingsSectionLabel>

          {unauthorized ? (
            <Card padded={false} className="p-8 text-center text-sm text-muted-foreground">
              <p className="font-medium text-text-secondary">{t('pairing.unpaired_title')}</p>
              <p className="mt-1">{t('pairing.unpaired_hint')}</p>
            </Card>
          ) : devices.length === 0 ? (
            <Card padded={false} className="p-8 text-center text-sm text-muted-foreground">
              {t('pairing.no_devices')}
            </Card>
          ) : (
            <SettingsListBody>
              {devices.map((device) => (
                <SettingsSelectableRow
                  key={device.id}
                  ariaLabel={device.name || device.id}
                  isSelected={selectedDeviceId === device.id}
                  onSelect={() => setSelectedDeviceId(device.id)}
                  leading={
                    <IconTile>
                      <Smartphone className="h-[18px] w-[18px] text-muted-foreground" />
                    </IconTile>
                  }
                  title={device.name || t('pairing.unnamed')}
                  subtitle={`${device.device_type || t('pairing.unknown')} · ${device.ip_address || '-'}`}
                  trailingIcon={<ChevronRight className="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />}
                />
              ))}
            </SettingsListBody>
          )}
        </SettingsPageShell>
      </div>

      <DetailPanelSurface open={selectedDevice !== null}>
        {selectedDevice && (
          <DetailPanel
            icon={
              <IconTile>
                <Smartphone className="h-[18px] w-[18px] text-muted-foreground" />
              </IconTile>
            }
            title={selectedDevice.name || t('pairing.unnamed')}
            subtitle={selectedDevice.device_type || t('pairing.unknown')}
            onClose={() => setSelectedDeviceId(null)}
            actions={
              <ActionMenu
                items={[
                  {
                    label: t('pairing.revoke'),
                    icon: <Trash2 />,
                    danger: true,
                    onClick: () => setPendingRevoke(selectedDevice),
                  },
                ]}
              />
            }
          >
            <div className="flex flex-col gap-5">
              <div>
                <DetailSectionTitle>{t('pairing.paired')}</DetailSectionTitle>
                <p className="mt-2 text-sm text-foreground/90">{fmtDate(selectedDevice.paired_at)}</p>
              </div>
              <div>
                <DetailSectionTitle>{t('pairing.last_seen')}</DetailSectionTitle>
                <p className="mt-2 text-sm text-foreground/90">
                  {fmtDate(selectedDevice.last_seen, { dateStyle: 'medium', timeStyle: 'medium' })}
                </p>
              </div>
              <div>
                <DetailSectionTitle>{t('pairing.ip')}</DetailSectionTitle>
                <p className="mt-2 font-mono text-sm text-muted-foreground">{selectedDevice.ip_address || '-'}</p>
              </div>
            </div>
          </DetailPanel>
        )}
      </DetailPanelSurface>

      <ConfirmDialog
        open={pendingRevoke !== null}
        danger
        title={t('pairing.revoke_title')}
        message={t('pairing.revoke_message', { value: pendingRevoke?.name || t('pairing.this_device') })}
        confirmLabel={t('pairing.revoke')}
        onConfirm={() => {
          if (pendingRevoke) void handleRevokeDevice(pendingRevoke.id);
        }}
        onClose={() => setPendingRevoke(null)}
      />
    </div>
  );
}
