# Why this branch exists

This branch is upstream `master` plus the fork's four `fork-*.yml` workflows
and nothing else. It carries no product code and is never merged anywhere.

GitHub resolves a `workflow_dispatch` against the ref you dispatch on, while
`Fork Test Branch` checks out whatever its `branch` input names. Those two are
separate, and this branch is the first one — so a feature branch is tested as
it is, with no copy of the workflow pushed beside it:

```sh
gh workflow run "Fork Test Branch" --repo metalmon/zeroclaw \
  --ref fork/ci -f branch=feat/<the branch under test> \
  -f features=agent-runtime,gateway,acp-bridge,channel-telegram,plugins-wasm
```

Drop `volt-lease` from `features` for anything based on upstream `master`: the
feature exists only on our own branches and cargo refuses the whole run.

Before this branch, every round of review got its own throwaway `ci/*` branch
carrying a copy of the workflow. Four of them once carried the whole assembled
`main` onto this public mirror — licence gate, authz and all — and sat there
for weeks. One branch with only CI config replaces that habit.

A deliberately failing run (tests committed before their fix, to watch them
fail) still needs a ref of its own, because that state must not appear on a
pull request. Use one reusable name, `ci/red`, and force-push over it. Never a
new branch per round.

Keep this branch rebased on upstream `master` when the workflows change; the
workflow files are the only thing that may differ from it.
