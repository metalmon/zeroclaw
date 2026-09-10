// Component/interaction tests for the Roles admin screen. Mirrors the
// react-test-renderer harness established by
// `src/contexts/sessionLifecycle.test.ts` / `src/components/layout/Sidebar.test.ts`
// (globalThis shims + a mocked `fetch`, no jsdom) rather than introducing a
// new testing-library dependency — this repo has none of those installed
// (see `web/package.json`), and every existing interaction test in the tree
// already uses this pattern (`.props.onClick()` / `.props.onChange()` calls
// via `react-test-renderer`, run under `node --experimental-strip-types --test`
// or `jiti`).
import assert from 'node:assert/strict';
import test from 'node:test';
import React, { createElement } from 'react';
import { act, create, type ReactTestInstance, type ReactTestRenderer } from 'react-test-renderer';

class MemoryStorage implements Storage {
  private readonly values = new Map<string, string>();

  get length(): number { return this.values.size; }
  clear(): void { this.values.clear(); }
  getItem(key: string): string | null { return this.values.get(key) ?? null; }
  key(index: number): string | null { return [...this.values.keys()][index] ?? null; }
  removeItem(key: string): void { this.values.delete(key); }
  setItem(key: string, value: string): void { this.values.set(key, value); }
}

const storage = new MemoryStorage();
storage.setItem('zeroclaw_token', 'admin-tok');

const fakeWindow = {
  location: { protocol: 'http:', host: 'localhost' },
  dispatchEvent: () => true,
  addEventListener: () => {},
  removeEventListener: () => {},
};
const fakeDocument = {
  activeElement: null as unknown,
  addEventListener: () => {},
  removeEventListener: () => {},
  createElement: () => ({ style: {}, focus: () => {}, select: () => {}, value: '' }),
  body: { appendChild: () => {}, removeChild: () => {} },
  execCommand: () => true,
};

Object.assign(globalThis, {
  React,
  localStorage: storage,
  window: fakeWindow,
  document: fakeDocument,
});
Object.defineProperty(globalThis, 'navigator', {
  configurable: true,
  value: { language: 'en-US' },
});
(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean })
  .IS_REACT_ACT_ENVIRONMENT = true;

// ── Fixture data + mock backend ──────────────────────────────────────────

interface FixtureProfile {
  id: string;
  allowed_agents: string[];
  admin: boolean;
}

interface FixturePrincipal {
  profiles: string[];
  device_ids: string[];
  token_hashes: string[];
  allowed_agents: string[];
}

let profiles: FixtureProfile[];
let principals: Record<string, FixturePrincipal>;
let agents: string[];
/** Calls captured for assertions: every non-GET request's method + path + body. */
let calls: Array<{ method: string; path: string; body: unknown }>;
/** When set, the NEXT create/update-profile POST/PUT gets this status instead
 *  of succeeding — used to exercise the 403-forbidden UI path. */
let nextProfileWriteStatus: number | null = null;

function resetFixtures(): void {
  profiles = [
    { id: 'crm', allowed_agents: ['crm-bot'], admin: false },
    { id: 'ops', allowed_agents: [], admin: true },
  ];
  principals = {
    alice: { profiles: [], device_ids: [], token_hashes: ['h1'], allowed_agents: [] },
    bob: { profiles: ['ops'], device_ids: ['d1'], token_hashes: [], allowed_agents: [] },
  };
  agents = ['crm-bot', 'hr-bot'];
  calls = [];
  nextProfileWriteStatus = null;
}

function json(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { 'content-type': 'application/json' },
  });
}

function principalListEntries(id: string): { entries: unknown[] } {
  const p = principals[id];
  if (!p) return { entries: [] };
  const mk = (suffix: string, value: unknown) => ({
    path: `authz.principals.${id}.${suffix}`,
    category: 'authz',
    kind: 'string-array',
    type_hint: 'Vec<String>',
    value,
    populated: true,
    is_secret: false,
  });
  return {
    entries: [
      mk('profiles', p.profiles),
      mk('device_ids', p.device_ids),
      mk('token_hashes', p.token_hashes),
      mk('allowed_agents', p.allowed_agents),
    ],
  };
}

globalThis.fetch = async (input, init) => {
  const url = typeof input === 'string' ? input : input instanceof URL ? input.toString() : input.url;
  const method = (init?.method ?? 'GET').toUpperCase();
  const u = new URL(url, 'http://local');
  const body = init?.body ? JSON.parse(String(init.body)) : undefined;

  if (method === 'GET' && u.pathname === '/api/authz/profiles') {
    return json({ profiles });
  }
  if (method === 'GET' && u.pathname === '/api/config/map-keys' && u.searchParams.get('path') === 'authz.principals') {
    return json({ path: 'authz.principals', keys: Object.keys(principals) });
  }
  if (method === 'GET' && u.pathname === '/api/config/list') {
    const prefix = u.searchParams.get('prefix') ?? '';
    const id = prefix.replace(/^authz\.principals\./, '');
    return json(principalListEntries(id));
  }
  if (method === 'GET' && u.pathname === '/api/agents') {
    return json({ agents });
  }

  calls.push({ method, path: u.pathname + u.search, body });

  if (method === 'POST' && u.pathname === '/api/authz/profiles') {
    if (nextProfileWriteStatus) {
      return json({ code: 'forbidden', error: 'nope' }, nextProfileWriteStatus);
    }
    const b = body as FixtureProfile;
    if (profiles.some((p) => p.id === b.id)) {
      return json({ code: 'conflict', message: 'already exists' }, 409);
    }
    profiles.push({ id: b.id, allowed_agents: b.allowed_agents, admin: b.admin });
    return json(b);
  }
  if (method === 'PUT' && u.pathname === '/api/authz/profiles') {
    if (nextProfileWriteStatus) {
      return json({ code: 'forbidden', error: 'nope' }, nextProfileWriteStatus);
    }
    const b = body as FixtureProfile;
    const idx = profiles.findIndex((p) => p.id === b.id);
    if (idx >= 0) profiles[idx] = b; else profiles.push(b);
    return json(b);
  }
  if (method === 'DELETE' && u.pathname === '/api/authz/profiles') {
    const id = u.searchParams.get('id')!;
    profiles = profiles.filter((p) => p.id !== id);
    const affected = Object.entries(principals)
      .filter(([, p]) => p.profiles.includes(id))
      .map(([pid]) => pid);
    return json({ id, deleted: true, affected_principals: affected });
  }
  const bindMatch = u.pathname.match(/^\/api\/authz\/principals\/([^/]+)\/profiles$/);
  if (bindMatch) {
    const id = decodeURIComponent(bindMatch[1]!);
    const p = principals[id];
    if (!p) return json({ code: 'not_found', message: 'no such principal' }, 404);
    if (method === 'PUT') {
      const profileId = (body as { profile_id: string }).profile_id;
      if (!p.profiles.includes(profileId)) p.profiles.push(profileId);
      return json({ principal_id: id, profiles: p.profiles });
    }
    if (method === 'DELETE') {
      const profileId = u.searchParams.get('profile_id')!;
      p.profiles = p.profiles.filter((x) => x !== profileId);
      return json({ principal_id: id, profiles: p.profiles });
    }
  }
  return json({ error: 'unhandled' }, 500);
};

const { default: Roles } = await import('./Roles.tsx');
const Select = (await import('@/components/ui/Select.tsx')).Select;

// ── Helpers ───────────────────────────────────────────────────────────────

function nodeText(node: ReactTestInstance): string {
  return node.children
    .map((child) => (typeof child === 'string' ? child : nodeText(child)))
    .join('');
}

function buttonWithText(renderer: ReactTestRenderer, text: string): ReactTestInstance {
  const match = renderer.root
    .findAllByType('button')
    .find((b) => nodeText(b).includes(text));
  assert.ok(match, `expected a <button> containing "${text}"`);
  return match;
}

async function mount(): Promise<ReactTestRenderer> {
  let renderer!: ReactTestRenderer;
  await act(async () => {
    renderer = create(createElement(Roles));
  });
  // Flush the three parallel fetches `useRoles` kicks off on mount.
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });
  return renderer;
}

// ── Tests ─────────────────────────────────────────────────────────────────

test('renders profiles and principals, flagging an unassigned principal as PENDING', async () => {
  resetFixtures();
  const renderer = await mount();

  const text = nodeText(renderer.root);
  assert.ok(text.includes('crm'), 'the crm profile id is rendered');
  assert.ok(text.includes('ops'), 'the ops profile id is rendered');
  assert.ok(text.includes('alice'), 'the alice principal id is rendered');
  assert.ok(text.includes('bob'), 'the bob principal id is rendered');
  assert.ok(text.includes('PENDING'), 'alice (empty profiles) is flagged PENDING');

  await act(async () => { renderer.unmount(); });
});

test('creating a profile posts allowed_agents=["*"] for the "all agents" toggle and refreshes the list', async () => {
  resetFixtures();
  const renderer = await mount();

  await act(async () => { buttonWithText(renderer, 'New Profile').props.onClick(); });

  const idInput = renderer.root.findByProps({ id: 'roles-profile-id' });
  await act(async () => {
    idInput.props.onChange({ target: { value: 'support-team' } });
  });

  const allAgentsLabel = renderer.root
    .findAllByType('label')
    .find((l) => nodeText(l).includes('All agents'));
  assert.ok(allAgentsLabel, 'the "All agents" checkbox label is rendered');
  const allAgentsCheckbox = allAgentsLabel.findByType('input');
  await act(async () => {
    allAgentsCheckbox.props.onChange({ target: { checked: true } });
  });

  await act(async () => { buttonWithText(renderer, 'Save').props.onClick(); });
  // Let the create call + its post-success refetch settle.
  await act(async () => { await Promise.resolve(); await Promise.resolve(); await Promise.resolve(); });

  const createCall = calls.find((c) => c.method === 'POST' && c.path === '/api/authz/profiles');
  assert.ok(createCall, 'a POST to /api/authz/profiles was made');
  assert.deepEqual(createCall!.body, { id: 'support-team', allowed_agents: ['*'], admin: false });

  assert.ok(
    nodeText(renderer.root).includes('support-team'),
    'the new profile appears in the list after the post-create refetch',
  );

  await act(async () => { renderer.unmount(); });
});

test('binding a pending principal to a profile calls the bind endpoint and clears PENDING', async () => {
  resetFixtures();
  const renderer = await mount();
  assert.ok(nodeText(renderer.root).includes('PENDING'));

  // Drive the (controlled) Select directly, the same way existing tests in
  // this repo drive a controlled <textarea> via `.props.onChange(...)`
  // rather than simulating the open-dropdown mouse interaction. Scope to
  // alice's <li> row specifically — bob also has an unbound profile (he's
  // only bound to "ops" of the two configured profiles) and so renders his
  // own bind picker too.
  const aliceRow = renderer.root
    .findAllByType('li')
    .find((li) => nodeText(li).includes('alice'));
  assert.ok(aliceRow, "alice's principal row is rendered");
  const aliceSelect = aliceRow.findByType(Select);
  await act(async () => { aliceSelect.props.onChange('crm'); });

  const aliceBindButton = aliceRow
    .findAllByType('button')
    .find((b) => nodeText(b).includes('Bind profile'));
  assert.ok(aliceBindButton, "alice's Bind profile button is rendered");
  await act(async () => { aliceBindButton.props.onClick(); });
  await act(async () => { await Promise.resolve(); await Promise.resolve(); await Promise.resolve(); });

  const bindCall = calls.find((c) => c.method === 'PUT' && c.path === '/api/authz/principals/alice/profiles');
  assert.ok(bindCall, 'a PUT to bind alice was made');
  assert.deepEqual(bindCall!.body, { profile_id: 'crm' });

  assert.ok(
    !nodeText(renderer.root).includes('PENDING'),
    'alice is no longer PENDING once bound (bob was already bound)',
  );

  await act(async () => { renderer.unmount(); });
});

test('a 403 from the admin gate surfaces the "requires admin access" message, not a raw error', async () => {
  resetFixtures();
  nextProfileWriteStatus = 403;
  const renderer = await mount();

  await act(async () => { buttonWithText(renderer, 'New Profile').props.onClick(); });
  const idInput = renderer.root.findByProps({ id: 'roles-profile-id' });
  await act(async () => { idInput.props.onChange({ target: { value: 'blocked' } }); });
  await act(async () => { buttonWithText(renderer, 'Save').props.onClick(); });
  await act(async () => { await Promise.resolve(); await Promise.resolve(); });

  assert.ok(
    nodeText(renderer.root).includes('requires a principal bound to an admin profile'),
    'the friendly admin-required message is shown instead of a raw 403/HttpError string',
  );
  assert.equal(profiles.some((p) => p.id === 'blocked'), false, 'the profile was NOT created');

  await act(async () => { renderer.unmount(); });
});
