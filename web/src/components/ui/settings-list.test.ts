import assert from "node:assert/strict";
import test from "node:test";
import React, { createElement } from "react";
import { create, act, type ReactTestRenderer } from "react-test-renderer";

import { SettingsSelectableRow } from "./settings-list.tsx";

// Match the harness the other react-test-renderer suites use (Roles.test.ts):
// enable act(...) and expose React globally for the classic JSX transform.
(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
(globalThis as typeof globalThis & { React: typeof React }).React = React;

function hasAriaPressed(renderer: ReactTestRenderer): boolean {
  return renderer.root.findAll((n) => n.props?.["aria-pressed"] !== undefined).length > 0;
}

test("selectable row renders an interactive toggle when onSelect is given", async () => {
  let r: ReactTestRenderer | undefined;
  await act(async () => {
    r = create(createElement(SettingsSelectableRow, { title: "role-a", ariaLabel: "role-a", onSelect: () => {} }));
  });
  assert.equal(hasAriaPressed(r!), true);
  await act(async () => { r!.unmount(); });
});

test("non-selectable row (no onSelect) renders no dead toggle button", async () => {
  let r: ReactTestRenderer | undefined;
  await act(async () => {
    r = create(createElement(SettingsSelectableRow, { title: "principal-a", ariaLabel: "principal-a" }));
  });
  assert.equal(hasAriaPressed(r!), false);
  await act(async () => { r!.unmount(); });
});
