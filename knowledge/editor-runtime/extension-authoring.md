# Extension Authoring Boundary

This guide explains when a task is still normal authoring and when it has crossed into extension work.

## Two different task types

### Authoring inside the public chain

Stay in the normal `author` chain when you are:

- creating or editing `.mei`
- reusing existing `.stock/components/**` or `.stock/templates/**`
- wiring datasets, metrics, templates, and layouts together
- fixing diagnostics in existing MeiLang apps

### Extension work outside the public chain

Treat the task as extension/platform authoring when you are:

- creating a brand new component pack
- creating a new renderer entry and `manifest.json`
- creating a new template pack or a reusable shell family
- introducing a new provider/capability boundary instead of only composing existing assets

## New component minimum skeleton

The current minimum component-pack shape is:

```text
_components/<pack>/
  manifest.json
  README.md
  *.js
```

Public rule:

- `manifest.json` registers stable component ids and script entrypoints.
- The renderer implementation stays in JS/Web Components today.
- Public authoring guidance should not pretend a new component exists until the pack is registered and discoverable.

## New template minimum skeleton

The current minimum template-pack shape is:

```text
.stock/templates/<pack>/
  README.md
  panel/
  metric-card/
  *.mei
```

Public rule:

- A template becomes public only after its path, base ids, and stable reuse contract are documented.
- Reusing cockpit templates is normal authoring.
- Inventing a new shell family is extension work until it has a stable contract.

## How AI should react

When the task is extension work, AI should:

1. explicitly say the task is leaving the normal `author` chain
2. switch from packaged public contracts to extension/profile docs
3. avoid claiming that an unregistered component or template is already public
4. treat JS/manifest/registry files as the source of truth for the new extension

## Public authoring stop signals

If any of these are true, stop pretending the public author chain is enough:

- the desired component id is absent from `component-contracts.json` and `manifest.json`
- the desired template path is absent from `template-contracts.json` or the template index
- the task requires changing renderer behavior instead of only passing public props
- the task introduces new capability/provider registration

## Recommended next docs

For extension work, continue with:

- `docs/mei-lang/basic/06-extension-capability-provider-profile.md`
- component/template pack implementation docs under `docs/mei-lang/implementation/extensions/`
