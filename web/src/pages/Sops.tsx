import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from 'react';
import {
  AlertTriangle,
  ChevronDown,
  ChevronRight,
  XCircle,
  Loader2,
  Plus,
  Save,
  Trash2,
  X,
} from 'lucide-react';
import { Link, useNavigate, useParams } from 'react-router-dom';
import { Badge, Card, PageHeader, HelpTip } from '@/components/ui';
import SopCanvas from './SopCanvas';
import MarkdownEditor from '@/components/MarkdownEditor';
import ToolPicker from '@/components/ToolPicker';
import { PlannedCallsEditor, JsonField } from '@/components/SopCalls';
import SopStepList from '@/components/SopStepList';
import { plural, t, enumLabel } from '@/lib/i18n';
import { loadAgentPickerSummaries } from '@/lib/agents';
import { getMapKeys } from '@/lib/api';
import {
  listSops,
  listRuns,
  getSopGraph,
  getRunOverlay,
  getSop,
  runSop,
  createSop,
  saveSop,
  deleteSop,
  wireDraft,
  graphDraft,
  triggerSources,
  sopFieldHelp,
  overlayCallsByStep,
  parseCondition,
  buildCondition,
  sopPriorities,
  sopExecutionModes,
  sopStepKinds,
  sopAdmissionPolicies,
  type WireRole,
  type SopSummary,
  type SopGraph,
  type RunOverlay,
  type Sop,
  type SopStep,
  type SopTrigger,
  type StepFailure,
  type StepSchema,
  type StepToolScope,
  type StepToolCall,
  type TriggerSourceRegistry,
  type BoundTriggerSource,
  type TriggerField,
  type PayloadContract,
  type ConditionOpSpec,
  type ConditionField,
} from '@/lib/sops';

function blankStep(number: number): SopStep {
  return {
    number,
    title: '',
    body: '',
    kind: 'execute',
    requires_confirmation: false,
    suggested_tools: [],
  };
}

const DRAFT_STORAGE_KEY = 'zeroclaw_sop_draft';
const DRAFT_EDITING_NAME_KEY = 'zeroclaw_sop_editing_name';

function setArgAtPath(
  root: Record<string, unknown>,
  segments: string[],
  value: string | null,
): Record<string, unknown> {
  const head = segments[0];
  if (head === undefined) return root;
  const rest = segments.slice(1);
  const next = { ...root };
  if (rest.length === 0) {
    if (value === null) delete next[head];
    else next[head] = value;
    return next;
  }
  const child = typeof next[head] === 'object' && next[head] !== null ? (next[head] as Record<string, unknown>) : {};
  next[head] = setArgAtPath(child, rest, value);
  return next;
}

function writeStepBinding(sop: Sop, toStep: number, toPin: string, value: string | null): Sop {
  const segments = toPin.split('.');
  if (segments[0] !== 'calls') return sop;
  const callIdx = Number(segments[1]);
  const argSegments = segments.slice(2);
  if (Number.isNaN(callIdx) || argSegments.length === 0) return sop;
  return {
    ...sop,
    steps: sop.steps.map((step) => {
      if (step.number !== toStep || !step.calls) return step;
      return {
        ...step,
        calls: step.calls.map((call, idx) => {
          if (idx !== callIdx) return call;
          const args = (call.args ?? {}) as Record<string, unknown>;
          return { ...call, args: setArgAtPath(args, argSegments, value) };
        }),
      };
    }),
  };
}

function loadStoredDraft(): Sop | null {
  try {
    const raw = sessionStorage.getItem(DRAFT_STORAGE_KEY);
    return raw ? (JSON.parse(raw) as Sop) : null;
  } catch {
    return null;
  }
}

function storeDraft(draft: Sop | null): void {
  try {
    if (draft) sessionStorage.setItem(DRAFT_STORAGE_KEY, JSON.stringify(draft));
    else sessionStorage.removeItem(DRAFT_STORAGE_KEY);
  } catch {
    // Storage is best-effort; a failure only loses cross-navigation recovery.
  }
}

function loadStoredEditingName(): string | null {
  try {
    return sessionStorage.getItem(DRAFT_EDITING_NAME_KEY);
  } catch {
    return null;
  }
}

function storeEditingName(name: string | null): void {
  try {
    if (name !== null) sessionStorage.setItem(DRAFT_EDITING_NAME_KEY, name);
    else sessionStorage.removeItem(DRAFT_EDITING_NAME_KEY);
  } catch {
    // Best-effort; a failure only degrades a post-reload rename into a fork.
  }
}

function blankSop(name: string): Sop {
  return {
    name,
    description: '',
    version: '1.0.0',
    priority: 'normal',
    execution_mode: 'supervised',
    triggers: [{ type: 'manual' }],
    steps: [blankStep(1)],
    cooldown_secs: 0,
    max_concurrent: 1,
    admission_policy: 'parallel',
    max_pending_approvals: 0,
    deterministic: false,
  };
}

function DiagnosticsPanel({ graph }: { graph: SopGraph }) {
  if (graph.diagnostics.length === 0) return null;
  return (
    <Card className="mt-4">
      <div className="mb-2 font-medium text-foreground">{t('sops.diagnostics')}</div>
      <ul className="space-y-1 text-sm">
        {graph.diagnostics.map((d, i) => (
          <li key={i} className="flex items-start gap-2">
            {d.severity === 'error' ? (
              <XCircle className="mt-0.5 h-4 w-4 shrink-0 text-status-error" aria-hidden />
            ) : (
              <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0 text-status-warning" aria-hidden />
            )}
            <span className="text-foreground">
              <span className="text-muted-foreground">
                {t('sops.step')} {d.step}:
              </span>{' '}
              {d.message}
            </span>
          </li>
        ))}
      </ul>
    </Card>
  );
}

const INPUT_CLS = 'w-full rounded border border-border bg-card px-2 py-1 text-foreground';

function StepBodyEditor({
  value,
  onChange,
}: {
  value: string;
  onChange: (next: string) => void;
}) {
  const [focused, setFocused] = useState(false);
  return (
    <div>
      <span className="mb-1 block text-muted-foreground text-sm">
        <HelpTip text={sopFieldHelp('SopStep', 'body')}>{t('sops.step_body_label')}</HelpTip>
      </span>
      <MarkdownEditor
        value={value}
        onChange={onChange}
        onFocus={() => setFocused(true)}
        onBlur={() => setFocused(false)}
        height={focused ? '20rem' : '4rem'}
        lineNumbers={focused}
        placeholder={t('sops.step_body_placeholder')}
      />
    </div>
  );
}

function Field({
  label,
  hint,
  help,
  children,
}: {
  label: string;
  hint?: string | null;
  help?: string | null;
  children: ReactNode;
}) {
  return (
    <label className="block text-sm">
      <span className="mb-1 block text-muted-foreground">
        {help ? <HelpTip text={help}>{label}</HelpTip> : label}
      </span>
      {children}
      {hint ? <p className="mt-1 text-xs text-text-faint">{hint}</p> : null}
    </label>
  );
}

function TextField({
  label,
  value,
  onChange,
  placeholder,
  help,
}: {
  label: string;
  value: string;
  onChange: (next: string) => void;
  placeholder?: string;
  help?: string | null;
}) {
  return (
    <Field label={label} help={help}>
      <input
        type="text"
        value={value}
        placeholder={placeholder}
        onChange={(e) => onChange(e.target.value)}
        className={INPUT_CLS}
      />
    </Field>
  );
}

function SelectField({
  label,
  value,
  onChange,
  options,
  disabled,
  children,
  help,
}: {
  label: string;
  value: string;
  onChange: (next: string) => void;
  options?: readonly string[];
  disabled?: boolean;
  children?: ReactNode;
  help?: string | null;
}) {
  return (
    <Field label={label} help={help}>
      <select
        value={value}
        onChange={(e) => onChange(e.target.value)}
        disabled={disabled}
        className={INPUT_CLS}
      >
        {children}
        {(options ?? []).map((opt) => (
          <option key={opt} value={opt}>
            {enumLabel(opt)}
          </option>
        ))}
      </select>
    </Field>
  );
}

/// Collapsible section wrapper shared by the step editor and the SOP header
/// form, so both surfaces speak the same accordion visual language instead
/// of each rolling its own disclosure widget.
function StepSection({
  title,
  icon,
  defaultOpen = false,
  collapsible = true,
  badge,
  headerExtra,
  children,
}: {
  title: string;
  icon?: ReactNode;
  defaultOpen?: boolean;
  collapsible?: boolean;
  badge?: ReactNode;
  headerExtra?: ReactNode;
  children: ReactNode;
}) {
  const [open, setOpen] = useState(defaultOpen);
  if (!collapsible) {
    return (
      <div className="mt-3 border-t border-border pt-3 first:mt-0 first:border-t-0 first:pt-0">
        <div className="mb-2 flex items-center gap-2 text-xs font-semibold text-foreground">
          {icon}
          <span>{title}</span>
          {badge}
        </div>
        <div className="space-y-2">{children}</div>
      </div>
    );
  }
  return (
    <div className="mt-3 border-t border-border pt-2 first:mt-0 first:border-t-0 first:pt-0">
      <div className="flex items-center gap-2">
        <button
          type="button"
          onClick={() => setOpen((o) => !o)}
          className="flex flex-1 items-center gap-1.5 py-1 text-left text-xs font-semibold text-foreground hover:text-primary"
        >
          {open ? (
            <ChevronDown className="h-3.5 w-3.5 shrink-0 text-muted-foreground" aria-hidden />
          ) : (
            <ChevronRight className="h-3.5 w-3.5 shrink-0 text-muted-foreground" aria-hidden />
          )}
          {icon}
          <span>{title}</span>
          {badge}
        </button>
        {headerExtra}
      </div>
      {open ? <div className="space-y-2 pb-1 pt-1">{children}</div> : null}
    </div>
  );
}

function failureKind(f: StepFailure | undefined): 'fail' | 'retry' | 'goto' {
  if (f === undefined || f === 'fail') return 'fail';
  if ('retry' in f) return 'retry';
  return 'goto';
}

function StepEditor({
  step,
  index,
  count,
  capturedCalls,
  approvalPolicyNames,
  onChange,
  onRemove,
  onMove,
  agentAliases,
  parentAgent,
}: {
  step: SopStep;
  index: number;
  count: number;
  capturedCalls?: StepToolCall[];
  approvalPolicyNames: string[];
  onChange: (patch: Partial<SopStep>) => void;
  onRemove: () => void;
  onMove: (dir: -1 | 1) => void;
  agentAliases: string[];
  parentAgent?: string | null;
}) {
  const rowRef = useRef<HTMLDivElement | null>(null);
  const routing = step.routing ?? {};
  const fkind = failureKind(step.on_failure);
  const setFailure = (kind: 'fail' | 'retry' | 'goto') => {
    if (kind === 'fail') onChange({ on_failure: 'fail' });
    else if (kind === 'retry') onChange({ on_failure: { retry: { max: 1 } } });
    else onChange({ on_failure: { goto: { step: 1 } } });
  };
  const setRouting = (patch: Partial<typeof routing>) =>
    onChange({ routing: { ...routing, ...patch } });

  const scope = step.scope ?? {};
  const setScope = (patch: Partial<StepToolScope>) => {
    const merged = { ...(step.scope ?? {}), ...patch };
    const allow = merged.allow && merged.allow.length > 0 ? merged.allow : undefined;
    const deny = merged.deny && merged.deny.length > 0 ? merged.deny : undefined;
    onChange({ scope: allow || deny ? { allow, deny } : undefined });
  };
  const setSchema = (patch: Partial<StepSchema>) => {
    const merged = { ...(step.schema ?? {}), ...patch };
    const hasInput = merged.input !== undefined && merged.input !== null;
    const hasOutput = merged.output !== undefined && merged.output !== null;
    onChange({ schema: hasInput || hasOutput ? merged : undefined });
  };

  const routingNonDefault =
    (routing.depends_on?.length ?? 0) > 0 ||
    routing.next != null ||
    !!routing.when ||
    (routing.switch?.length ?? 0) > 0;
  const routingDefaultOpen = routingNonDefault || fkind !== 'fail';
  const hitlDefaultOpen = step.kind === 'checkpoint' || (step.requires_confirmation ?? false);
  const policyListId = `sop-policy-options-${step.number}`;

  return (
    <div
      ref={rowRef}
      className="rounded-[var(--radius-lg)] border border-border bg-card p-3"
    >
      <div className="mb-2 flex items-center gap-2">
        <HelpTip text={sopFieldHelp('SopStep', 'number')}>
          <span className="inline-flex h-6 w-6 items-center justify-center rounded bg-primary text-xs font-semibold text-primary-foreground">
            {step.number}
          </span>
        </HelpTip>
        <input
          type="text"
          value={step.title}
          onChange={(e) => onChange({ title: e.target.value })}
          placeholder={t('sops.step_title_placeholder')}
          title={sopFieldHelp('SopStep', 'title') ?? undefined}
          className="flex-1 rounded border border-border bg-card px-2 py-1 text-sm text-foreground"
        />
        <select
          value={step.kind ?? 'execute'}
          onChange={(e) => {
            const kind = e.target.value as SopStep['kind'];
            // Leaving the capability kind clears its now-orphaned config so it
            // isn't silently serialized into the SOP (the Capability section is
            // hidden for non-capability kinds).
            onChange(
              kind === 'capability'
                ? { kind }
                : { kind, capability: undefined, with: undefined },
            );
          }}
          className="rounded border border-border bg-card px-1.5 py-1 text-xs text-foreground"
          aria-label={t('sops.step_kind')}
          title={sopFieldHelp('SopStep', 'kind') ?? undefined}
        >
          {sopStepKinds.map((kind) => (
            <option key={kind} value={kind}>
              {t(`sops.kind_${kind}`)}
            </option>
          ))}
        </select>
        <button
          type="button"
          onClick={() => onMove(-1)}
          disabled={index === 0}
          className="rounded px-1.5 py-1 text-muted-foreground hover:bg-secondary disabled:opacity-30"
          aria-label={t('sops.move_up')}
        >
          ↑
        </button>
        <button
          type="button"
          onClick={() => onMove(1)}
          disabled={index === count - 1}
          className="rounded px-1.5 py-1 text-muted-foreground hover:bg-secondary disabled:opacity-30"
          aria-label={t('sops.move_down')}
        >
          ↓
        </button>
        <button
          type="button"
          onClick={onRemove}
          className="rounded px-1.5 py-1 text-status-error hover:bg-secondary"
          aria-label={t('sops.remove_step')}
        >
          <Trash2 className="h-4 w-4" aria-hidden />
        </button>
      </div>

      <StepSection title={t('sops.section_instructions')} collapsible={false}>
        <StepBodyEditor
          value={step.body}
          onChange={(next) => onChange({ body: next })}
        />
        <div>
          <span className="mb-1 block text-muted-foreground text-sm">
            <HelpTip text={sopFieldHelp('SopStep', 'agent')}>{t('sops.step_agent_label')}</HelpTip>
          </span>
          <select
            value={step.agent ?? ''}
            onChange={(e) => onChange({ agent: e.target.value === '' ? null : e.target.value })}
            className="w-full rounded border border-border bg-card px-2 py-1 text-sm text-foreground"
          >
            <option value="">
              {t('sops.step_agent_inherit')}
              {parentAgent ? ` (${parentAgent})` : ''}
            </option>
            {agentAliases.map((alias) => (
              <option key={alias} value={alias}>
                {alias}
              </option>
            ))}
          </select>
        </div>
      </StepSection>

      <StepSection title={t('sops.section_tools')} defaultOpen>
        <div className="text-xs">
          <span className="mb-1 block text-muted-foreground">
            <HelpTip text={sopFieldHelp('SopStep', 'suggested_tools')}>
              {t('sops.step_tools_label')}
            </HelpTip>
          </span>
          <ToolPicker
            value={step.suggested_tools ?? []}
            onChange={(next) => onChange({ suggested_tools: next })}
            agent={step.agent ?? parentAgent ?? undefined}
          />
        </div>
        <PlannedCallsEditor
          calls={step.calls ?? []}
          captured={capturedCalls}
          agent={step.agent ?? parentAgent}
          onChange={(next) => onChange({ calls: next })}
        />
      </StepSection>

      <StepSection
        title={t('sops.scope_precise_toggle')}
        defaultOpen={(step.scope?.allow?.length ?? 0) > 0 || (step.scope?.deny?.length ?? 0) > 0}
      >
        <div className="space-y-2 text-xs">
          {(step.scope?.allow?.length ?? 0) > 0 ? (
            <p className="text-text-faint">{t('sops.scope_overrides_hint')}</p>
          ) : null}
          <div>
            <span className="mb-1 block text-muted-foreground">
              <HelpTip text={sopFieldHelp('StepToolScope', 'allow')}>
                {t('sops.scope_allow')}
              </HelpTip>
            </span>
            <ToolPicker
              id={`sop-scope-allow-${step.number}`}
              value={scope.allow ?? []}
              onChange={(next) => setScope({ allow: next })}
              agent={step.agent ?? parentAgent ?? undefined}
            />
          </div>
          <div>
            <span className="mb-1 block text-muted-foreground">
              <HelpTip text={sopFieldHelp('StepToolScope', 'deny')}>
                {t('sops.scope_deny')}
              </HelpTip>
            </span>
            <ToolPicker
              id={`sop-scope-deny-${step.number}`}
              value={scope.deny ?? []}
              onChange={(next) => setScope({ deny: next })}
              agent={step.agent ?? parentAgent ?? undefined}
            />
          </div>
        </div>
      </StepSection>

      <StepSection title={t('sops.section_routing')} defaultOpen={routingDefaultOpen}>
        <div className="grid grid-cols-3 gap-2 text-xs">
          <TextField
            label={t('sops.routing_depends_on')}
            value={(routing.depends_on ?? []).join(', ')}
            placeholder="2, 3"
            help={sopFieldHelp('StepRouting', 'depends_on')}
            onChange={(v) =>
              setRouting({
                depends_on: v
                  .split(',')
                  .map((s) => parseInt(s.trim(), 10))
                  .filter((n) => Number.isFinite(n)),
              })
            }
          />
          <Field label={t('sops.routing_next')} help={sopFieldHelp('StepRouting', 'next')}>
            <input
              type="number"
              value={routing.next ?? ''}
              onChange={(e) =>
                setRouting({ next: e.target.value ? parseInt(e.target.value, 10) : undefined })
              }
              placeholder="→"
              className={INPUT_CLS}
            />
          </Field>
          <TextField
            label={t('sops.routing_when')}
            value={routing.when ?? ''}
            placeholder={t('sops.routing_when_placeholder')}
            help={sopFieldHelp('StepRouting', 'when')}
            onChange={(v) => setRouting({ when: v || undefined })}
          />
          <SelectField
            label={t('sops.on_failure')}
            value={fkind}
            help={sopFieldHelp('SopStep', 'on_failure')}
            onChange={(v) => setFailure(v as 'fail' | 'retry' | 'goto')}
          >
            <option value="fail">{t('sops.failure_fail')}</option>
            <option value="retry">{t('sops.failure_retry')}</option>
            <option value="goto">{t('sops.failure_goto')}</option>
          </SelectField>
          {fkind === 'retry' && step.on_failure && typeof step.on_failure === 'object' && 'retry' in step.on_failure ? (
            <Field label={t('sops.failure_max')}>
              <input
                type="number"
                value={step.on_failure.retry.max}
                onChange={(e) =>
                  onChange({ on_failure: { retry: { max: parseInt(e.target.value, 10) || 1 } } })
                }
                className={INPUT_CLS}
              />
            </Field>
          ) : null}
          {fkind === 'goto' && step.on_failure && typeof step.on_failure === 'object' && 'goto' in step.on_failure ? (
            <Field label={t('sops.failure_goto_step')}>
              <input
                type="number"
                value={step.on_failure.goto.step}
                onChange={(e) =>
                  onChange({ on_failure: { goto: { step: parseInt(e.target.value, 10) || 1 } } })
                }
                className={INPUT_CLS}
              />
            </Field>
          ) : null}
        </div>
        <div className="rounded border border-border p-2">
          <div className="mb-1 flex items-center justify-between">
            <span className="text-xs font-medium text-foreground">
              <HelpTip text={sopFieldHelp('StepRouting', 'switch')}>{t('sops.switch_ports')}</HelpTip>
            </span>
            <button
              type="button"
              onClick={() =>
                setRouting({
                  switch: [...(routing.switch ?? []), { name: `port ${(routing.switch?.length ?? 0) + 1}`, when: undefined, goto: undefined }],
                })
              }
              className="rounded border border-border px-2 py-0.5 text-xs text-foreground hover:bg-secondary"
            >
              <Plus className="mr-1 inline h-3 w-3" aria-hidden />
              {t('sops.add_port')}
            </button>
          </div>
          {(routing.switch ?? []).length === 0 ? (
            <div className="text-xs text-text-faint">{t('sops.no_ports')}</div>
          ) : (
            (routing.switch ?? []).map((rule, ri) => {
              const setRule = (patch: Partial<typeof rule>) => {
                const rules = [...(routing.switch ?? [])];
                rules[ri] = { ...rules[ri]!, ...patch };
                setRouting({ switch: rules });
              };
              return (
                <div key={ri} className="mb-1 grid grid-cols-[1fr_1.4fr_4rem_1.5rem] items-center gap-1">
                  <input
                    type="text"
                    value={rule.name}
                    onChange={(e) => setRule({ name: e.target.value })}
                    placeholder={t('sops.port_name')}
                    className="rounded border border-border bg-card px-1.5 py-0.5 text-xs text-foreground"
                  />
                  <input
                    type="text"
                    value={rule.when ?? ''}
                    onChange={(e) => setRule({ when: e.target.value || undefined })}
                    placeholder={t('sops.port_when')}
                    className="rounded border border-border bg-card px-1.5 py-0.5 text-xs text-foreground"
                  />
                  <input
                    type="number"
                    value={rule.goto ?? ''}
                    onChange={(e) => setRule({ goto: e.target.value ? parseInt(e.target.value, 10) : undefined })}
                    placeholder="→"
                    className="rounded border border-border bg-card px-1.5 py-0.5 text-xs text-foreground"
                  />
                  <button
                    type="button"
                    onClick={() => setRouting({ switch: (routing.switch ?? []).filter((_, j) => j !== ri) })}
                    className="text-status-error"
                    aria-label={t('sops.remove_port')}
                  >
                    <Trash2 className="h-3.5 w-3.5" aria-hidden />
                  </button>
                </div>
              );
            })
          )}
        </div>
      </StepSection>

      <StepSection
        title={t('sops.section_approval')}
        defaultOpen={hitlDefaultOpen}
        headerExtra={
          <label
            className="ml-auto flex shrink-0 items-center gap-1 text-xs font-normal text-muted-foreground"
            onClick={(e) => e.stopPropagation()}
          >
            <input
              type="checkbox"
              checked={step.requires_confirmation ?? false}
              onChange={(e) => onChange({ requires_confirmation: e.target.checked })}
            />
            <HelpTip text={sopFieldHelp('SopStep', 'requires_confirmation')}>
              {t('sops.requires_confirmation')}
            </HelpTip>
          </label>
        }
      >
        <Field label={t('sops.step_policy_label')} help={sopFieldHelp('SopStep', 'policy')}>
          <input
            type="text"
            list={policyListId}
            value={step.policy ?? ''}
            placeholder={t('sops.step_policy_placeholder')}
            onChange={(e) => onChange({ policy: e.target.value === '' ? undefined : e.target.value })}
            className={INPUT_CLS}
          />
          <datalist id={policyListId}>
            {approvalPolicyNames.map((name) => (
              <option key={name} value={name} />
            ))}
          </datalist>
        </Field>
        <Field label={t('sops.step_gate_prompt_label')} help={sopFieldHelp('SopStep', 'gate_prompt')}>
          <textarea
            value={step.gate_prompt ?? ''}
            placeholder={t('sops.step_gate_prompt_placeholder')}
            rows={3}
            onChange={(e) =>
              onChange({ gate_prompt: e.target.value === '' ? undefined : e.target.value })
            }
            className={INPUT_CLS}
          />
        </Field>
        <TextField
          label={t('sops.step_edit_label')}
          value={step.edit ?? ''}
          placeholder={t('sops.step_edit_placeholder')}
          help={sopFieldHelp('SopStep', 'edit')}
          onChange={(v) => onChange({ edit: v === '' ? undefined : v })}
        />
      </StepSection>

      {step.kind === 'capability' ? (
        <StepSection title={t('sops.section_capability')} collapsible={false}>
          <TextField
            label={t('sops.step_capability_label')}
            value={step.capability ?? ''}
            placeholder={t('sops.step_capability_placeholder')}
            help={sopFieldHelp('SopStep', 'capability')}
            onChange={(v) => onChange({ capability: v === '' ? undefined : v })}
          />
          {!step.capability ? (
            <p className="text-xs text-status-warning">{t('sops.step_capability_required_hint')}</p>
          ) : null}
          <JsonField
            label={t('sops.step_with_label')}
            help={sopFieldHelp('SopStep', 'with')}
            value={step.with}
            onChange={(next) => onChange({ with: next })}
            rows={4}
          />
        </StepSection>
      ) : null}

      <StepSection title={t('sops.section_schema')} defaultOpen={false}>
        <JsonField
          label={t('sops.step_schema_input')}
          help={sopFieldHelp('StepSchema', 'input')}
          value={step.schema?.input}
          onChange={(next) => setSchema({ input: next })}
          rows={4}
        />
        <JsonField
          label={t('sops.step_schema_output')}
          help={sopFieldHelp('StepSchema', 'output')}
          value={step.schema?.output}
          onChange={(next) => setSchema({ output: next })}
          rows={4}
        />
      </StepSection>
    </div>
  );
}

const CHANNEL_SOURCE = 'channel';
const MANUAL_SOURCE = 'manual';

function triggerSource(trigger: SopTrigger): string {
  return trigger.type === CHANNEL_SOURCE ? CHANNEL_SOURCE : trigger.type;
}

/// Blank value for a registry field, shaped by its declared kind. No field
/// names are consulted: the registry's `kind` is the single authority.
function blankFieldValue(field: TriggerField): unknown {
  switch (field.kind) {
    case 'list':
      return [];
    case 'expression':
      return null;
    default:
      return '';
  }
}

/// Build a fresh trigger for a chosen source. The registry supplies the field
/// set; each bound field starts at its kind's blank value and the channel
/// source starts on the first walked channel kind. No per-source field logic
/// is hardcoded.
function blankTrigger(
  source: string,
  registry: TriggerSourceRegistry | null,
): SopTrigger {
  if (source === CHANNEL_SOURCE) {
    const firstChannel = registry?.channels[0]?.channel ?? '';
    return { type: 'channel', channel: firstChannel, alias: null, condition: null };
  }
  if (source === MANUAL_SOURCE) return { type: 'manual' };
  const bound = registry?.bound.find((b) => b.source === source);
  const base: Record<string, unknown> = { type: source };
  for (const field of bound?.fields ?? []) {
    base[field.name] = blankFieldValue(field);
  }
  return base as unknown as SopTrigger;
}

/// i18n lookup with a fallback: `t()` returns the key itself when no
/// translation exists, so detect that and fall back instead of leaking keys.
function tOr(key: string, fallback: string | null): string | null {
  const value = t(key);
  return value === key ? fallback : value;
}

function triggerFieldLabel(field: string): string {
  return tOr(`sops.trigger_${field}`, field) ?? field;
}

function triggerFieldHint(field: string): string | null {
  return tOr(`sops.trigger_${field}_hint`, null);
}

function triggerFieldPlaceholder(field: string): string {
  return tOr(`sops.trigger_${field}_placeholder`, null) ?? '';
}

function TriggerFieldInput({
  field,
  value,
  onChange,
}: {
  field: TriggerField;
  value: unknown;
  onChange: (next: unknown) => void;
}) {
  const name = field.name;
  const options = field.options ?? [];
  const hint = triggerFieldHint(name);
  const help = sopFieldHelp('SopTrigger', name);

  if (options.length > 0) {
    if (field.multi) {
      const selected = new Set(Array.isArray(value) ? (value as string[]) : []);
      const toggle = (opt: string) => {
        const next = new Set(selected);
        if (next.has(opt)) next.delete(opt);
        else next.add(opt);
        onChange(options.filter((o) => next.has(o)));
      };
      return (
        <fieldset className="block text-sm">
          <legend className="mb-1 block text-muted-foreground">
            {help ? (
              <HelpTip text={help}>{triggerFieldLabel(name)}</HelpTip>
            ) : (
              triggerFieldLabel(name)
            )}
          </legend>
          <div className="flex flex-wrap gap-2">
            {options.map((opt) => (
              <label
                key={opt}
                className="inline-flex items-center gap-1.5 rounded border border-border px-2 py-1 text-xs text-foreground"
              >
                <input
                  type="checkbox"
                  checked={selected.has(opt)}
                  onChange={() => toggle(opt)}
                />
                {opt}
              </label>
            ))}
          </div>
          {hint ? <p className="mt-1 text-xs text-text-faint">{hint}</p> : null}
        </fieldset>
      );
    }
    const current = typeof value === 'string' ? value : '';
    return (
      <Field label={triggerFieldLabel(name)} hint={hint} help={help}>
        <select value={current} onChange={(e) => onChange(e.target.value)} className={INPUT_CLS}>
          {options.map((opt) => (
            <option key={opt} value={opt}>
              {opt}
            </option>
          ))}
        </select>
      </Field>
    );
  }

  const isList = field.kind === 'list';
  const isExpression = field.kind === 'expression';
  const text = isList
    ? Array.isArray(value)
      ? value.join(', ')
      : ''
    : typeof value === 'string'
      ? value
      : '';
  return (
    <Field label={triggerFieldLabel(name)} hint={hint} help={help}>
      <input
        type="text"
        value={text}
        placeholder={triggerFieldPlaceholder(name)}
        onChange={(e) => {
          const raw = e.target.value;
          if (isList) {
            onChange(
              raw
                .split(',')
                .map((s) => s.trim())
                .filter((s) => s.length > 0),
            );
          } else if (isExpression) {
            onChange(raw.length > 0 ? raw : null);
          } else {
            onChange(raw);
          }
        }}
        className={INPUT_CLS}
      />
    </Field>
  );
}

function conditionValueInputType(vt: ConditionField['value_type'] | undefined): string {
  if (vt === 'number') return 'number';
  if (vt === 'date_time') return 'datetime-local';
  return 'text';
}

/// Guided condition builder. Users pick a payload field (from the source's
/// walked contract), an operator (from the registry catalog), and a value;
/// the three assemble into the `$.path op value` string the engine evaluates.
/// No operator or path is typed blind. Every channel and known-shape source
/// enumerates its fields, so the builder renders a field picker. Only genuinely
/// arbitrary payloads (mqtt, amqp) mark `open` and fall back to a free input
/// with an advanced raw-string escape hatch; `direct` scalar payloads drop the
/// path entirely. Sources with no contract render nothing (condition
/// unsupported).
function ConditionBuilder({
  contract,
  operators,
  value,
  onChange,
}: {
  contract: PayloadContract | null | undefined;
  operators: ConditionOpSpec[];
  value: string | null;
  onChange: (next: string | null) => void;
}) {
  const parsed = parseCondition(value, operators);
  const [raw, setRaw] = useState(false);
  if (!contract) return null;

  const fields = contract.fields ?? [];
  const isDirect = contract.direct === true;
  const isOpen = contract.open === true && fields.length === 0 && !isDirect;
  const selectedField = fields.find((f) => `${f.path}` === parsed.path);
  const valueType = selectedField?.value_type;

  const emit = (part: { path: string | null; op: string; value: string }) =>
    onChange(buildCondition(part));

  if (raw && isOpen) {
    return (
      <div className="space-y-1">
        <Field
          label={t('sops.trigger_condition')}
          hint={t('sops.condition_raw_hint')}
          help={sopFieldHelp('SopTrigger', 'condition')}
        >
          <input
            type="text"
            value={parsed.raw}
            placeholder={t('sops.trigger_condition_placeholder')}
            onChange={(e) => onChange(e.target.value.length > 0 ? e.target.value : null)}
            className={INPUT_CLS}
          />
        </Field>
        <button
          type="button"
          onClick={() => setRaw(false)}
          className="text-xs text-muted-foreground underline hover:text-primary"
        >
          {t('sops.condition_use_builder')}
        </button>
      </div>
    );
  }

  return (
    <fieldset className="space-y-2">
      <legend className="mb-1 block text-sm text-muted-foreground">
        <HelpTip text={sopFieldHelp('SopTrigger', 'condition')}>
          {t('sops.trigger_condition')}
        </HelpTip>
      </legend>
      <div className="grid grid-cols-[1.4fr_auto_1.4fr] items-end gap-2">
        {isDirect ? (
          <div className="text-xs text-text-faint">{t('sops.condition_direct_payload')}</div>
        ) : isOpen ? (
          <Field label={t('sops.condition_field')}>
            <input
              type="text"
              value={parsed.path ?? ''}
              placeholder={t('sops.condition_field_placeholder')}
              onChange={(e) =>
                emit({ path: e.target.value, op: parsed.op, value: parsed.value })
              }
              className={INPUT_CLS}
            />
          </Field>
        ) : (
          <Field label={t('sops.condition_field')}>
            <select
              value={parsed.path ?? ''}
              onChange={(e) => emit({ path: e.target.value, op: parsed.op, value: parsed.value })}
              className={INPUT_CLS}
            >
              <option value="">{t('sops.condition_pick_field')}</option>
              {fields.map((f) => (
                <option key={f.path} value={f.path}>
                  {f.label}
                </option>
              ))}
            </select>
          </Field>
        )}
        <Field label={t('sops.condition_operator')}>
          <select
            value={parsed.op}
            onChange={(e) =>
              emit({ path: parsed.path, op: e.target.value, value: parsed.value })
            }
            className={INPUT_CLS}
          >
            <option value="">{t('sops.condition_any')}</option>
            {operators.map((op) => (
              <option key={op.token} value={op.token}>
                {op.label} ({op.token})
              </option>
            ))}
          </select>
        </Field>
        {selectedField?.options && selectedField.options.length > 0 ? (
          <Field label={t('sops.condition_value')}>
            <select
              value={parsed.value}
              onChange={(e) =>
                emit({ path: parsed.path, op: parsed.op, value: e.target.value })
              }
              className={INPUT_CLS}
            >
              <option value="">{t('sops.condition_pick_value')}</option>
              {selectedField.options.map((opt) => (
                <option key={opt} value={opt}>
                  {opt}
                </option>
              ))}
            </select>
          </Field>
        ) : (
          <Field label={t('sops.condition_value')}>
            <input
              type={conditionValueInputType(valueType)}
              value={parsed.value}
              placeholder={t('sops.condition_value_placeholder')}
              onChange={(e) =>
                emit({ path: parsed.path, op: parsed.op, value: e.target.value })
              }
              className={INPUT_CLS}
            />
          </Field>
        )}
      </div>
      {isOpen ? (
        <button
          type="button"
          onClick={() => setRaw(true)}
          className="text-xs text-muted-foreground underline hover:text-primary"
        >
          {t('sops.condition_use_raw')}
        </button>
      ) : null}
    </fieldset>
  );
}

function ChannelTriggerFields({
  trigger,
  registry,
  onChange,
}: {
  trigger: Extract<SopTrigger, { type: 'channel' }>;
  registry: TriggerSourceRegistry | null;
  onChange: (patch: Partial<Extract<SopTrigger, { type: 'channel' }>>) => void;
}) {
  const channels = registry?.channels ?? [];
  const selected = channels.find((c) => c.channel === trigger.channel);
  return (
    <div className="space-y-2">
      <div className="grid grid-cols-2 gap-3">
        <SelectField
          label={t('sops.trigger_channel')}
          value={trigger.channel}
          onChange={(v) => onChange({ channel: v, alias: null })}
          options={channels.map((c) => c.channel)}
          help={sopFieldHelp('SopTrigger', 'channel')}
        />
        <SelectField
          label={t('sops.trigger_alias')}
          value={trigger.alias ?? ''}
          onChange={(v) => onChange({ alias: v.length > 0 ? v : null })}
          disabled={!selected?.configured}
          options={(selected?.aliases ?? []).map((a) => a.alias)}
          help={sopFieldHelp('SopTrigger', 'alias')}
        >
          <option value="">{t('sops.trigger_alias_any')}</option>
        </SelectField>
      </div>
      {selected && !selected.configured ? (
        <div className="flex items-center gap-2 text-xs text-status-warning">
          <AlertTriangle className="h-3.5 w-3.5" aria-hidden />
          <span>{t('sops.trigger_unconfigured')}</span>
          <Link
            to={selected.setup_path}
            className="underline hover:text-primary"
          >
            {t('sops.trigger_setup_link')}
          </Link>
        </div>
      ) : null}
      <ConditionBuilder
        contract={selected?.condition}
        operators={registry?.operators ?? []}
        value={trigger.condition}
        onChange={(next) => onChange({ condition: next })}
      />
    </div>
  );
}

function TriggerEditor({
  trigger,
  index,
  selected,
  registry,
  onChange,
  onRemove,
}: {
  trigger: SopTrigger;
  index: number;
  selected: boolean;
  registry: TriggerSourceRegistry | null;
  onChange: (next: SopTrigger) => void;
  onRemove: () => void;
}) {
  const source = triggerSource(trigger);
  const bound = registry?.bound ?? [];
  const boundFields: TriggerField[] =
    source === CHANNEL_SOURCE || source === MANUAL_SOURCE
      ? []
      : (bound.find((b) => b.source === source)?.fields ?? []);
  const boundContract: PayloadContract | null =
    source === CHANNEL_SOURCE || source === MANUAL_SOURCE
      ? null
      : (bound.find((b) => b.source === source)?.condition ?? null);

  const sources: string[] = registry?.sources ?? [
    ...bound.map((b: BoundTriggerSource) => b.source),
    CHANNEL_SOURCE,
  ];

  return (
    <div
      className={`space-y-2 rounded border bg-card p-2 ${
        selected ? 'border-primary' : 'border-border'
      }`}
    >
      <div className="flex items-center justify-between gap-2">
        <div className="flex-1">
          <SelectField
            label={t('sops.trigger_source')}
            value={source}
            onChange={(v) => onChange(blankTrigger(v, registry))}
            options={sources}
          />
        </div>
        <button
          type="button"
          onClick={onRemove}
          className="mt-5 inline-flex items-center rounded border border-border p-1 text-muted-foreground hover:bg-secondary"
          aria-label={t('sops.remove_trigger')}
        >
          <Trash2 className="h-3.5 w-3.5" aria-hidden />
        </button>
      </div>
      {trigger.type === CHANNEL_SOURCE ? (
        <ChannelTriggerFields
          trigger={trigger}
          registry={registry}
          onChange={(patch) => onChange({ ...trigger, ...patch })}
        />
      ) : source === MANUAL_SOURCE ? (
        <p className="text-xs text-muted-foreground">{t('sops.trigger_manual_hint')}</p>
      ) : (
        <div className="space-y-2">
          {boundFields.map((field) => (
            <TriggerFieldInput
              key={field.name}
              field={field}
              value={(trigger as unknown as Record<string, unknown>)[field.name]}
              onChange={(next) =>
                onChange({
                  ...(trigger as unknown as Record<string, unknown>),
                  [field.name]: next,
                } as unknown as SopTrigger)
              }
            />
          ))}
          {boundContract ? (
            <ConditionBuilder
              contract={boundContract}
              operators={registry?.operators ?? []}
              value={
                ((trigger as unknown as Record<string, unknown>).condition as string | null) ??
                null
              }
              onChange={(next) =>
                onChange({
                  ...(trigger as unknown as Record<string, unknown>),
                  condition: next,
                } as unknown as SopTrigger)
              }
            />
          ) : null}
        </div>
      )}
      <span className="sr-only">{`trigger ${index + 1}`}</span>
    </div>
  );
}

function StepListRow({
  step,
  index,
  count,
  selected,
  onSelect,
  onMove,
  onRemove,
}: {
  step: SopStep;
  index: number;
  count: number;
  selected: boolean;
  onSelect: () => void;
  onMove: (dir: -1 | 1) => void;
  onRemove: () => void;
}) {
  return (
    <div
      className={`flex items-center gap-2 rounded border px-2 py-1.5 ${
        selected ? 'border-primary ring-1 ring-primary' : 'border-border'
      }`}
    >
      <button type="button" onClick={onSelect} className="flex min-w-0 flex-1 items-center gap-2 text-left">
        <span className="inline-flex h-5 w-5 shrink-0 items-center justify-center rounded bg-primary text-[11px] font-semibold text-primary-foreground">
          {step.number}
        </span>
        <span className="truncate text-sm text-foreground">{step.title || t('sops.untitled')}</span>
        {step.kind === 'checkpoint' ? <Badge tone="warn">⏸</Badge> : null}
        {step.calls && step.calls.length > 0 ? (
          <span className="shrink-0 text-[11px] text-muted-foreground">⚙ {step.calls.length}</span>
        ) : null}
      </button>
      <button
        type="button"
        onClick={() => onMove(-1)}
        disabled={index === 0}
        className="rounded px-1 text-muted-foreground hover:bg-secondary disabled:opacity-30"
        aria-label={t('sops.move_up')}
      >
        ↑
      </button>
      <button
        type="button"
        onClick={() => onMove(1)}
        disabled={index === count - 1}
        className="rounded px-1 text-muted-foreground hover:bg-secondary disabled:opacity-30"
        aria-label={t('sops.move_down')}
      >
        ↓
      </button>
      <button
        type="button"
        onClick={onRemove}
        className="rounded px-1 text-status-error hover:bg-secondary"
        aria-label={t('sops.remove_step')}
      >
        <Trash2 className="h-3.5 w-3.5" aria-hidden />
      </button>
    </div>
  );
}

function DraftSidebar({
  draft,
  saving,
  saveError,
  selectedStep,
  selectedTrigger,
  triggerRegistry,
  agentAliases,
  onSelectStep,
  onField,
  onTrigger,
  onAddTrigger,
  onRemoveTrigger,
  onAddStep,
  onRemoveStep,
  onMoveStep,
  onSave,
  onCancel,
}: {
  draft: Sop;
  saving: boolean;
  saveError: string | null;
  selectedStep: number | null;
  selectedTrigger: number | null;
  triggerRegistry: TriggerSourceRegistry | null;
  agentAliases: string[];
  onSelectStep: (n: number) => void;
  onField: (patch: Partial<Sop>) => void;
  onTrigger: (i: number, next: SopTrigger) => void;
  onAddTrigger: () => void;
  onRemoveTrigger: (i: number) => void;
  onAddStep: () => void;
  onRemoveStep: (i: number) => void;
  onMoveStep: (i: number, dir: -1 | 1) => void;
  onSave: () => void;
  onCancel: () => void;
}) {
  return (
    <Card className="space-y-3">
      {/* Header row must survive a much longer title than the English source
          (e.g. Russian "Редактировать СОП") inside the fixed 20rem sidebar
          column. min-w-0 + truncate lets the title give way instead of
          wrapping onto the Cancel/Save buttons; flex-shrink-0 keeps those
          buttons at full, always-readable size. */}
      <div className="flex items-center justify-between gap-2">
        <div className="min-w-0 truncate font-medium text-foreground" title={t('sops.editor_title')}>
          {t('sops.editor_title')}
        </div>
        <div className="flex flex-shrink-0 gap-2">
          <button
            type="button"
            onClick={onCancel}
            className="inline-flex items-center gap-1 rounded border border-border px-2 py-1 text-sm text-foreground hover:bg-secondary"
          >
            <X className="h-4 w-4" aria-hidden /> {t('sops.cancel')}
          </button>
          <button
            type="button"
            onClick={onSave}
            disabled={saving}
            className="inline-flex items-center gap-1 rounded bg-primary px-2 py-1 text-sm text-primary-foreground hover:bg-accent-light disabled:opacity-50"
          >
            {saving ? (
              <Loader2 className="h-4 w-4 animate-spin" aria-hidden />
            ) : (
              <Save className="h-4 w-4" aria-hidden />
            )}
            {t('sops.save')}
          </button>
        </div>
      </div>
      {saveError ? <div className="text-sm text-status-error">{saveError}</div> : null}
      <TextField
        label={t('sops.field_name')}
        value={draft.name}
        onChange={(v) => onField({ name: v })}
        help={sopFieldHelp('Sop', 'name')}
      />
      <TextField
        label={t('sops.field_description')}
        value={draft.description}
        onChange={(v) => onField({ description: v })}
        help={sopFieldHelp('Sop', 'description')}
      />
      <div className="grid grid-cols-2 gap-3">
        <TextField
          label={t('sops.field_version')}
          value={draft.version}
          onChange={(v) => onField({ version: v })}
          help={sopFieldHelp('Sop', 'version')}
        />
        <SelectField
          label={t('sops.field_priority')}
          value={draft.priority}
          onChange={(v) => onField({ priority: v as Sop['priority'] })}
          help={sopFieldHelp('Sop', 'priority')}
        >
          {sopPriorities.map((p) => (
            <option key={p} value={p}>
              {t(`sops.priority_${p}`)}
            </option>
          ))}
        </SelectField>
      </div>
      <SelectField
        label={t('sops.field_execution_mode')}
        value={draft.execution_mode}
        onChange={(v) => onField({ execution_mode: v as Sop['execution_mode'] })}
        help={sopFieldHelp('Sop', 'execution_mode')}
      >
        {sopExecutionModes.map((m) => (
          <option key={m} value={m}>
            {t(`sops.exec_${m}`)}
          </option>
        ))}
      </SelectField>
      <SelectField
        label={t('sops.field_agent')}
        value={draft.agent ?? ''}
        onChange={(v) => onField({ agent: v === '' ? null : v })}
        help={sopFieldHelp('Sop', 'agent')}
      >
        <option value="">{t('sops.agent_none')}</option>
        {agentAliases.map((alias) => (
          <option key={alias} value={alias}>
            {alias}
          </option>
        ))}
      </SelectField>
      <StepSection title={t('sops.section_scheduling')} defaultOpen={false}>
        <Field
          label={t('sops.field_cooldown_secs')}
          help={sopFieldHelp('Sop', 'cooldown_secs')}
          hint={t('sops.field_cooldown_secs_hint')}
        >
          <input
            type="number"
            min={0}
            value={draft.cooldown_secs}
            onChange={(e) =>
              onField({ cooldown_secs: Math.max(0, parseInt(e.target.value, 10) || 0) })
            }
            className={INPUT_CLS}
          />
        </Field>
        <Field label={t('sops.field_max_concurrent')} help={sopFieldHelp('Sop', 'max_concurrent')}>
          <input
            type="number"
            min={1}
            value={draft.max_concurrent}
            onChange={(e) =>
              onField({ max_concurrent: Math.max(1, parseInt(e.target.value, 10) || 1) })
            }
            className={INPUT_CLS}
          />
        </Field>
        <SelectField
          label={t('sops.field_admission_policy')}
          value={draft.admission_policy ?? 'parallel'}
          help={sopFieldHelp('Sop', 'admission_policy')}
          onChange={(v) => onField({ admission_policy: v as Sop['admission_policy'] })}
        >
          {sopAdmissionPolicies.map((p) => (
            <option key={p} value={p}>
              {t(`sops.admission_${p}`)}
            </option>
          ))}
        </SelectField>
        <Field
          label={t('sops.field_max_pending_approvals')}
          help={sopFieldHelp('Sop', 'max_pending_approvals')}
          hint={t('sops.unlimited_hint')}
        >
          <input
            type="number"
            min={0}
            value={draft.max_pending_approvals ?? 0}
            onChange={(e) =>
              onField({ max_pending_approvals: Math.max(0, parseInt(e.target.value, 10) || 0) })
            }
            className={INPUT_CLS}
          />
        </Field>
        <label className="flex items-center gap-2 text-sm text-muted-foreground">
          <input
            type="checkbox"
            checked={draft.deterministic}
            onChange={(e) => onField({ deterministic: e.target.checked })}
          />
          <HelpTip text={sopFieldHelp('Sop', 'deterministic')}>
            {t('sops.field_deterministic')}
          </HelpTip>
        </label>
      </StepSection>
      <div className="space-y-2">
        <div className="flex items-center justify-between">
          <span className="text-sm font-medium text-foreground">{t('sops.triggers')}</span>
          <button
            type="button"
            onClick={onAddTrigger}
            className="inline-flex items-center gap-1 rounded border border-border px-2 py-1 text-xs text-foreground hover:bg-secondary"
          >
            <Plus className="h-3.5 w-3.5" aria-hidden /> {t('sops.add_trigger')}
          </button>
        </div>
        {draft.triggers.length === 0 ? (
          <p className="text-xs text-muted-foreground">{t('sops.trigger_none')}</p>
        ) : (
          draft.triggers.map((trigger, i) => (
            <TriggerEditor
              key={i}
              trigger={trigger}
              index={i}
              selected={selectedTrigger === i}
              registry={triggerRegistry}
              onChange={(next) => onTrigger(i, next)}
              onRemove={() => onRemoveTrigger(i)}
            />
          ))
        )}
      </div>
      <div className="space-y-2">
        <div className="flex items-center justify-between">
          <span className="text-sm font-medium text-foreground">{t('sops.steps')}</span>
          <button
            type="button"
            onClick={onAddStep}
            className="inline-flex items-center gap-1 rounded border border-border px-2 py-1 text-xs text-foreground hover:bg-secondary"
          >
            <Plus className="h-3.5 w-3.5" aria-hidden /> {t('sops.add_step')}
          </button>
        </div>
        {draft.steps.length === 0 ? (
          <p className="text-xs text-muted-foreground">{t('sops.no_steps')}</p>
        ) : (
          draft.steps.map((s, i) => (
            <StepListRow
              key={i}
              step={s}
              index={i}
              count={draft.steps.length}
              selected={selectedStep === s.number}
              onSelect={() => onSelectStep(s.number)}
              onMove={(dir) => onMoveStep(i, dir)}
              onRemove={() => onRemoveStep(i)}
            />
          ))
        )}
      </div>
    </Card>
  );
}

function StepInspector({
  draft,
  selectedStep,
  runCallsByStep,
  agentAliases,
  approvalPolicyNames,
  onStep,
  onRemoveStep,
  onMoveStep,
}: {
  draft: Sop;
  selectedStep: number | null;
  runCallsByStep: Map<number, StepToolCall[]>;
  agentAliases: string[];
  approvalPolicyNames: string[];
  onStep: (i: number, patch: Partial<SopStep>) => void;
  onRemoveStep: (i: number) => void;
  onMoveStep: (i: number, dir: -1 | 1) => void;
}) {
  const index = draft.steps.findIndex((s) => s.number === selectedStep);
  const step = index >= 0 ? draft.steps[index] : undefined;
  if (!step) {
    return (
      <Card>
        <p className="text-sm text-muted-foreground">{t('sops.inspector_empty')}</p>
      </Card>
    );
  }
  return (
    <StepEditor
      // Remount per selected step so section-local UI state (accordion open/
      // closed, the scope-editor toggle) doesn't leak from the previously
      // inspected step onto this one.
      key={step.number}
      step={step}
      index={index}
      count={draft.steps.length}
      capturedCalls={runCallsByStep.get(step.number)}
      approvalPolicyNames={approvalPolicyNames}
      onChange={(patch) => onStep(index, patch)}
      onRemove={() => onRemoveStep(index)}
      onMove={(dir) => onMoveStep(index, dir)}
      agentAliases={agentAliases}
      parentAgent={draft.agent}
    />
  );
}

const noop = () => {};

/// Build a Manual-run payload skeleton from a SOP's step-1 input JSON Schema.
/// Registry-driven: keys and placeholder value shapes come from the SOP's own
/// declared `schema.input`, never a hardcoded per-SOP template.
function payloadSkeleton(sop: Sop | null): string {
  const input = sop?.steps?.find((s) => s.number === 1)?.schema?.input;
  const props =
    input && typeof input === 'object' && !Array.isArray(input)
      ? (input as { properties?: Record<string, unknown> }).properties
      : undefined;
  if (!props || typeof props !== 'object') return '{}';
  const skeleton: Record<string, unknown> = {};
  for (const [key, spec] of Object.entries(props)) {
    const type =
      spec && typeof spec === 'object' && !Array.isArray(spec)
        ? (spec as { type?: string }).type
        : undefined;
    skeleton[key] = placeholderForType(type);
  }
  return JSON.stringify(skeleton, null, 2);
}

function placeholderForType(type: string | undefined): unknown {
  switch (type) {
    case 'number':
    case 'integer':
      return 0;
    case 'boolean':
      return false;
    case 'array':
      return [];
    case 'object':
      return {};
    default:
      return '';
  }
}

/// Manual-run affordance for a SOP that declares a manual trigger. Fires
/// POST /api/sops/{name}/run and navigates to the run detail page so the
/// existing overlay route animates the run.
function ManualRunPanel({ name, sop }: { name: string; sop: Sop | null }) {
  const navigate = useNavigate();
  const [payload, setPayload] = useState('');
  const [running, setRunning] = useState(false);
  const [runError, setRunError] = useState<string | null>(null);

  const hasManualTrigger = useMemo(
    () => (sop?.triggers ?? []).some((tr) => tr.type === 'manual'),
    [sop],
  );

  useEffect(() => {
    if (!hasManualTrigger) {
      setPayload('');
      return;
    }
    setPayload((cur) => (cur.trim() ? cur : payloadSkeleton(sop)));
  }, [hasManualTrigger, sop]);

  const onRun = useCallback(() => {
    const trimmed = payload.trim();
    if (trimmed) {
      try {
        JSON.parse(trimmed);
      } catch {
        setRunError(`${t('sops.run_error')}: ${t('sops.invalid_json')}`);
        return;
      }
    }
    setRunning(true);
    setRunError(null);
    runSop(name, trimmed || undefined)
      .then(({ run_id }) =>
        navigate(`/runs/${encodeURIComponent(name)}/${encodeURIComponent(run_id)}`),
      )
      .catch((e: unknown) => setRunError(`${t('sops.run_error')}: ${String(e)}`))
      .finally(() => setRunning(false));
  }, [name, payload, navigate]);

  if (!hasManualTrigger) return null;

  return (
    <Card className="space-y-2">
      <textarea
        value={payload}
        onChange={(e) => setPayload(e.target.value)}
        placeholder={t('sops.run_payload_placeholder')}
        rows={4}
        className="w-full rounded border border-border bg-card px-2 py-1 font-mono text-xs text-foreground"
      />
      <div className="flex flex-wrap items-center gap-2">
        <button
          type="button"
          onClick={onRun}
          disabled={running}
          className="inline-flex items-center gap-1 rounded border border-border bg-primary px-3 py-1 text-sm font-medium text-primary-foreground hover:opacity-90 disabled:opacity-40"
        >
          {running ? <Loader2 className="h-4 w-4 animate-spin" aria-hidden /> : null}
          {t('sops.run')}
        </button>
        {runError ? <span className="text-xs text-status-error">{runError}</span> : null}
      </div>
    </Card>
  );
}

// ── /sops ── read-only collection navigator. No selection, graph, overlay, or
// mutation lives here; rows link to the addressable member view. Create is an
// addressable action (/sops/new), not inline state.
export function SopsList() {
  const [sops, setSops] = useState<SopSummary[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    listSops()
      .then((list) => {
        if (active) setSops(list);
      })
      .catch((e: unknown) => {
        if (active) setError(e instanceof Error ? e.message : String(e));
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => {
      active = false;
    };
  }, []);

  return (
    <div className="p-6 space-y-6 animate-fade-in">
      <PageHeader
        title={t('sops.title')}
        description={t('sops.subtitle')}
        actions={
          <Link
            to="/sops/new"
            className="inline-flex items-center gap-1 rounded bg-primary px-3 py-1.5 text-sm text-primary-foreground hover:bg-accent-light"
          >
            <Plus className="h-4 w-4" aria-hidden /> {t('sops.new')}
          </Link>
        }
      />
      {error ? (
        <Card>
          <div className="text-status-error">{error}</div>
        </Card>
      ) : loading ? (
        <Card>
          <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" aria-hidden />
        </Card>
      ) : sops.length === 0 ? (
        <Card>
          <div className="text-muted-foreground">{t('sops.empty')}</div>
        </Card>
      ) : (
        <Card className="p-2">
          <ul className="space-y-1">
            {sops.map((s) => (
              <li key={s.name}>
                <Link
                  to={`/sops/${encodeURIComponent(s.name)}`}
                  className="block rounded px-3 py-2 text-sm text-foreground hover:bg-secondary"
                >
                  <div className="font-medium">{s.name}</div>
                  {s.description ? (
                    <div className="truncate text-xs text-muted-foreground">{s.description}</div>
                  ) : null}
                </Link>
              </li>
            ))}
          </ul>
        </Card>
      )}
    </div>
  );
}

// ── /sops/:name ── read-only member representation. Renders the SOP graph via
// the same graph/get RPC the editor loads from, with no run-overlay tint: run
// progress belongs to /runs, not the SOP resource. Edit and Delete are
// addressable member actions, never inline editing.
export function SopView() {
  const { name = '' } = useParams();
  const navigate = useNavigate();
  const [graph, setGraph] = useState<SopGraph | null>(null);
  const [viewSop, setViewSop] = useState<Sop | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [layer, setLayer] = useState<'visual' | 'fields'>('visual');

  useEffect(() => {
    if (!name) return;
    let active = true;
    setLoading(true);
    Promise.all([getSopGraph(name), getSop(name)])
      .then(([g, full]) => {
        if (!active) return;
        setGraph(g);
        setViewSop(full);
      })
      .catch((e: unknown) => {
        if (!active) return;
        setError(e instanceof Error ? e.message : String(e));
        setGraph(null);
        setViewSop(null);
      })
      .finally(() => {
        if (active) setLoading(false);
      });
    return () => {
      active = false;
    };
  }, [name]);

  const onDelete = useCallback(() => {
    deleteSop(name)
      .then(() => navigate('/sops'))
      .catch((e: unknown) => setError(e instanceof Error ? e.message : String(e)));
  }, [name, navigate]);

  return (
    <div className="p-6 space-y-6 animate-fade-in">
      <PageHeader
        title={name}
        description={viewSop?.description || t('sops.subtitle')}
        actions={
          <Link to="/sops" className="text-sm text-primary hover:underline">
            {t('sops.back_to_list')}
          </Link>
        }
      />
      {error ? (
        <Card>
          <div className="text-status-error">{error}</div>
        </Card>
      ) : null}
      <ManualRunPanel name={name} sop={viewSop} />
      <div>
        <div className="mb-3 flex flex-wrap items-center gap-2">
          {graph ? (
            <Badge tone="neutral">
              {plural(graph.nodes.length, 'sops.steps_count')}
            </Badge>
          ) : null}
          <div className="ml-auto flex flex-wrap gap-2">
            <button
              type="button"
              onClick={() => setLayer((l) => (l === 'visual' ? 'fields' : 'visual'))}
              disabled={!graph}
              className="rounded border border-border px-2 py-1 text-sm text-foreground hover:bg-secondary disabled:opacity-40"
            >
              {layer === 'visual' ? t('sops.layer_fields') : t('sops.layer_visual')}
            </button>
            <Link
              to={`/sops/${encodeURIComponent(name)}/edit`}
              className="rounded border border-border px-2 py-1 text-sm text-foreground hover:bg-secondary"
            >
              {t('sops.edit')}
            </Link>
            <button
              type="button"
              onClick={onDelete}
              className="inline-flex items-center gap-1 rounded border border-border px-2 py-1 text-sm text-status-error hover:bg-secondary"
            >
              <Trash2 className="h-4 w-4" aria-hidden /> {t('sops.delete')}
            </button>
          </div>
        </div>
        {loading ? (
          <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" aria-hidden />
        ) : graph ? (
          <>
            {layer === 'visual' && viewSop ? (
              <SopCanvas
                draft={viewSop}
                graph={graph}
                selectedStep={null}
                readOnly
                onSelectStep={noop}
                onSelectTrigger={noop}
                onAddStep={noop}
                onConnect={noop}
                onDisconnect={noop}
                onConnectData={noop}
                onDisconnectData={noop}
              />
            ) : (
              <SopStepList graph={graph} />
            )}
            <DiagnosticsPanel graph={graph} />
          </>
        ) : null}
      </div>
    </div>
  );
}

// ── /sops/new and /sops/:name/edit ── the authoring surface. The draft is the
// single source of edit state; graph projection comes from graphDraft/wireDraft
// (never re-derived client-side). Captured-calls overlay is loaded from the
// SOP's latest run to feed the pin-from-run flow in the step inspector.
export function SopEditor() {
  const { name: routeName } = useParams();
  const editingRoute = routeName ?? null;
  const navigate = useNavigate();

  const [draft, setDraft] = useState<Sop | null>(loadStoredDraft);
  const [editingName, setEditingName] = useState<string | null>(loadStoredEditingName);
  const [draftGraph, setDraftGraph] = useState<SopGraph | null>(null);
  // Undo stack of pre-mutation draft snapshots. Every canvas edit (wire
  // connect/disconnect, data binding, node move) snapshots the draft here
  // before it mutates, so a single stack undoes them all uniformly.
  const undoStackRef = useRef<Sop[]>([]);
  const [undoDepth, setUndoDepth] = useState(0);
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [selectedStep, setSelectedStep] = useState<number | null>(null);
  const [selectedTrigger, setSelectedTrigger] = useState<number | null>(null);
  const [triggerRegistry, setTriggerRegistry] = useState<TriggerSourceRegistry | null>(null);
  const [agentAliases, setAgentAliases] = useState<string[]>([]);
  const [approvalPolicyNames, setApprovalPolicyNames] = useState<string[]>([]);
  const [latestOverlay, setLatestOverlay] = useState<RunOverlay | null>(null);

  // Load the draft the route addresses: an existing SOP by name for edit, or a
  // blank draft for new. A session-mirrored draft under the same identity wins
  // so navigating away (e.g. to configure a trigger channel) and back is lossless.
  useEffect(() => {
    let active = true;
    if (editingRoute === null) {
      setDraft((cur) => (cur && editingName === null ? cur : blankSop('')));
      setEditingName(null);
      return;
    }
    if (draft && editingName === editingRoute) return;
    getSop(editingRoute)
      .then((full) => {
        if (!active) return;
        setEditingName(full.name);
        setDraft(full);
      })
      .catch((e: unknown) => {
        if (active) setSaveError(e instanceof Error ? e.message : String(e));
      });
    return () => {
      active = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [editingRoute]);

  useEffect(() => {
    storeDraft(draft);
    if (!draft) return;
    const warn = (e: BeforeUnloadEvent) => {
      e.preventDefault();
      e.returnValue = '';
    };
    window.addEventListener('beforeunload', warn);
    return () => window.removeEventListener('beforeunload', warn);
  }, [draft]);

  useEffect(() => {
    storeEditingName(draft ? editingName : null);
  }, [draft, editingName]);

  useEffect(() => {
    let active = true;
    triggerSources()
      .then((reg) => {
        if (active) setTriggerRegistry(reg);
      })
      .catch(() => {});
    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    let active = true;
    loadAgentPickerSummaries()
      .then((list) => {
        if (active) setAgentAliases(list.map((a) => a.alias));
      })
      .catch(() => {});
    return () => {
      active = false;
    };
  }, []);

  // Named approval policies (`[sop.approval.policies.<name>]`) for the step
  // policy combobox. Best-effort: an empty/failed load still leaves the
  // input usable as free text.
  useEffect(() => {
    let active = true;
    getMapKeys('sop.approval.policies')
      .then((res) => {
        if (active) setApprovalPolicyNames(res.keys);
      })
      .catch(() => {});
    return () => {
      active = false;
    };
  }, []);

  // Captured-calls overlay from the SOP's latest run, one-shot. Feeds the step
  // inspector's pin-from-run flow only; live run watching is the run page's job.
  useEffect(() => {
    setLatestOverlay(null);
    if (editingRoute === null) return;
    let active = true;
    listRuns(editingRoute)
      .then((runs) => {
        const latest = runs.sort((a, b) => b.started_at.localeCompare(a.started_at))[0];
        if (!latest) return null;
        return getRunOverlay(editingRoute, latest.run_id);
      })
      .then((o) => {
        if (active && o) setLatestOverlay(o);
      })
      .catch(() => {});
    return () => {
      active = false;
    };
  }, [editingRoute]);

  const runCallsByStep = useMemo(() => overlayCallsByStep(latestOverlay), [latestOverlay]);

  // Snapshot the current draft before a canvas mutation so it can be undone.
  const pushUndo = useCallback((snapshot: Sop) => {
    const stack = undoStackRef.current;
    stack.push(snapshot);
    // Bound the history so a long editing session cannot grow unbounded.
    if (stack.length > 50) stack.shift();
    setUndoDepth(stack.length);
  }, []);

  const undo = useCallback(() => {
    const prev = undoStackRef.current.pop();
    setUndoDepth(undoStackRef.current.length);
    if (!prev) return;
    setSaveError(null);
    setDraft(prev);
    graphDraft(prev)
      .then(setDraftGraph)
      .catch((e: unknown) => setSaveError(e instanceof Error ? e.message : String(e)));
  }, []);

  // Ctrl+Z / Cmd+Z undoes the last canvas edit while an editor is open, unless
  // the user is typing in an input where the browser's native undo should win.
  useEffect(() => {
    if (!draft) return;
    const onKey = (e: KeyboardEvent) => {
      if (!(e.key === 'z' || e.key === 'Z') || !(e.ctrlKey || e.metaKey) || e.shiftKey) return;
      const target = e.target as HTMLElement | null;
      const tag = target?.tagName;
      if (tag === 'INPUT' || tag === 'TEXTAREA' || target?.isContentEditable) return;
      e.preventDefault();
      undo();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [draft, undo]);

  const onConnect = useCallback(
    (from: number, to: number, kind: WireRole, portIndex?: number) => {
      setDraft((d) => {
        if (!d) return d;
        pushUndo(d);
        wireDraft(d, { op: 'connect', from, to, role: kind, port: portIndex })
          .then((res) => {
            setDraft(res.sop);
            setDraftGraph(res.graph);
          })
          .catch((e: unknown) => setSaveError(e instanceof Error ? e.message : String(e)));
        return d;
      });
    },
    [pushUndo],
  );

  const onDisconnect = useCallback(
    (from: number, to: number, kind: WireRole, portIndex?: number) => {
      setDraft((d) => {
        if (!d) return d;
        pushUndo(d);
        wireDraft(d, { op: 'disconnect', from, to, role: kind, port: portIndex })
          .then((res) => {
            setDraft(res.sop);
            setDraftGraph(res.graph);
          })
          .catch((e: unknown) => setSaveError(e instanceof Error ? e.message : String(e)));
        return d;
      });
    },
    [pushUndo],
  );

  const onConnectData = useCallback(
    (fromStep: number, fromPin: string, toStep: number, toPin: string) => {
      const binding = `{{steps.${fromStep}.${fromPin}}}`;
      setDraft((d) => {
        if (!d) return d;
        pushUndo(d);
        const next = writeStepBinding(d, toStep, toPin, binding);
        graphDraft(next)
          .then((g) => {
            setDraft(next);
            setDraftGraph(g);
          })
          .catch((e: unknown) => setSaveError(e instanceof Error ? e.message : String(e)));
        return next;
      });
    },
    [pushUndo],
  );

  const onDisconnectData = useCallback(
    (toStep: number, toPin: string) => {
      setDraft((d) => {
        if (!d) return d;
        pushUndo(d);
        const next = writeStepBinding(d, toStep, toPin, null);
        graphDraft(next)
          .then((g) => {
            setDraft(next);
            setDraftGraph(g);
          })
          .catch((e: unknown) => setSaveError(e instanceof Error ? e.message : String(e)));
        return next;
      });
    },
    [pushUndo],
  );

  useEffect(() => {
    if (!draft) {
      setDraftGraph(null);
      return;
    }
    let active = true;
    graphDraft(draft)
      .then((g) => {
        if (active) setDraftGraph(g);
      })
      .catch((e: unknown) => {
        if (active) setSaveError(e instanceof Error ? e.message : String(e));
      });
    return () => {
      active = false;
    };
  }, [draft]);

  useEffect(() => {
    if (!draft) {
      setSelectedStep(null);
      return;
    }
    setSelectedStep((cur) =>
      cur !== null && draft.steps.some((s) => s.number === cur)
        ? cur
        : (draft.steps[0]?.number ?? null),
    );
  }, [draft]);

  const mutateDraft = useCallback((updater: (d: Sop) => Sop) => {
    setSaveError(null);
    setDraft((d) => (d ? updater(d) : d));
  }, []);

  const close = useCallback(
    (toName?: string) => {
      undoStackRef.current = [];
      setUndoDepth(0);
      setDraft(null);
      setEditingName(null);
      navigate(toName ? `/sops/${encodeURIComponent(toName)}` : '/sops');
    },
    [navigate],
  );

  const onSaveDraft = useCallback(() => {
    if (!draft) return;
    setSaving(true);
    setSaveError(null);
    // Name authority, three cases:
    //   - new draft (no editing name): create, 409 if the name already exists,
    //     so a new SOP can never silently overwrite an existing one.
    //   - edit under the same name: upsert via PUT (never 409s on itself).
    //   - rename (name diverged from the editing name): rejected. A rename is
    //     not a save; it must be an explicit operation so it cannot fork or
    //     clobber an unrelated SOP through a swallowed delete.
    const isNew = editingName === null;
    if (!isNew && draft.name !== editingName) {
      setSaving(false);
      setSaveError(`rename not supported: '${editingName}' cannot be saved as '${draft.name}'`);
      return;
    }
    const write = isNew ? createSop(draft) : saveSop(draft);
    const savedName = draft.name;
    write
      .then(() => close(savedName))
      .catch((e: unknown) => setSaveError(e instanceof Error ? e.message : String(e)))
      .finally(() => setSaving(false));
  }, [draft, editingName, close]);

  const editorHandlers = draft
    ? {
        onField: (patch: Partial<Sop>) => mutateDraft((d) => ({ ...d, ...patch })),
        onTrigger: (i: number, next: SopTrigger) =>
          mutateDraft((d) => ({
            ...d,
            triggers: d.triggers.map((tr, j) => (j === i ? next : tr)),
          })),
        onAddTrigger: () =>
          mutateDraft((d) => ({
            ...d,
            triggers: [...d.triggers, blankTrigger(MANUAL_SOURCE, triggerRegistry)],
          })),
        onRemoveTrigger: (i: number) =>
          mutateDraft((d) => ({ ...d, triggers: d.triggers.filter((_, j) => j !== i) })),
        onStep: (i: number, patch: Partial<SopStep>) =>
          mutateDraft((d) => ({
            ...d,
            steps: d.steps.map((s, j) => (j === i ? { ...s, ...patch } : s)),
          })),
        onMoveNode: (step: number, x: number, y: number) =>
          mutateDraft((d) => {
            pushUndo(d);
            return {
              ...d,
              steps: d.steps.map((s) => (s.number === step ? { ...s, pos: { x, y } } : s)),
            };
          }),
        onAddStep: () =>
          mutateDraft((d) => ({ ...d, steps: [...d.steps, blankStep(d.steps.length + 1)] })),
        onRemoveStep: (i: number) =>
          mutateDraft((d) => ({ ...d, steps: d.steps.filter((_, j) => j !== i) })),
        onMoveStep: (i: number, dir: -1 | 1) =>
          mutateDraft((d) => {
            const j = i + dir;
            if (j < 0 || j >= d.steps.length) return d;
            const steps = [...d.steps];
            [steps[i], steps[j]] = [steps[j]!, steps[i]!];
            return { ...d, steps };
          }),
      }
    : null;

  if (!draft || !editorHandlers) {
    return (
      <div className="p-6">
        <Card>
          <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" aria-hidden />
        </Card>
      </div>
    );
  }

  return (
    <div className="p-6 space-y-6 animate-fade-in">
      <PageHeader title={t('sops.editor_title')} description={t('sops.subtitle')} />
      <div className="grid grid-cols-1 items-start gap-4 lg:grid-cols-[20rem_minmax(0,1fr)]">
        <DraftSidebar
          draft={draft}
          saving={saving}
          saveError={saveError}
          selectedStep={selectedStep}
          selectedTrigger={selectedTrigger}
          triggerRegistry={triggerRegistry}
          agentAliases={agentAliases}
          onSelectStep={setSelectedStep}
          onField={editorHandlers.onField}
          onTrigger={editorHandlers.onTrigger}
          onAddTrigger={editorHandlers.onAddTrigger}
          onRemoveTrigger={editorHandlers.onRemoveTrigger}
          onAddStep={editorHandlers.onAddStep}
          onRemoveStep={editorHandlers.onRemoveStep}
          onMoveStep={editorHandlers.onMoveStep}
          onSave={onSaveDraft}
          onCancel={() => close(editingName ?? undefined)}
        />
        <div className="min-w-0 space-y-4">
          <StepInspector
            draft={draft}
            selectedStep={selectedStep}
            runCallsByStep={runCallsByStep}
            agentAliases={agentAliases}
            approvalPolicyNames={approvalPolicyNames}
            onStep={editorHandlers.onStep}
            onRemoveStep={editorHandlers.onRemoveStep}
            onMoveStep={editorHandlers.onMoveStep}
          />
          {draftGraph ? (
            <SopCanvas
              draft={draft}
              graph={draftGraph}
              selectedStep={selectedStep}
              onSelectStep={setSelectedStep}
              onSelectTrigger={setSelectedTrigger}
              onAddStep={editorHandlers.onAddStep}
              onRemoveStep={(n) => {
                const i = draft.steps.findIndex((s) => s.number === n);
                if (i >= 0) editorHandlers.onRemoveStep(i);
              }}
              onConnect={onConnect}
              onDisconnect={onDisconnect}
              onConnectData={onConnectData}
              onDisconnectData={onDisconnectData}
              onMoveNode={editorHandlers.onMoveNode}
              onUndo={undo}
              canUndo={undoDepth > 0}
            />
          ) : null}
        </div>
      </div>
    </div>
  );
}
