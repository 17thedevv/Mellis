# STD-ARCH-01 — Compatibility Contract Matrix

Status: canonical surfaces and deliberately preserved compatibility paths, as of
Phase 4G (Compatibility & Dead-Path Cleanup).

## Surfaces

| Surface | Canonical | Legacy read accepted | Canonical write |
| :--- | :--- | :--- | :--- |
| Source | `.ln` | `.ms` (read-only; probed after `.ln`) | `.ln` only |
| Artifact | `.llib` (magic `LLIB`) | `.mlib` (reader also accepts magic `MLIB`) | `.llib` only |
| Provider identity | manifest logical ID (e.g. `core/panic`) | no legacy aliases | canonical only |
| Runtime ABI | `__mellis_*` | n/a | frozen ABI |

## Resolver precedence

- discovery: `.llib` → `.ln` → `package/package.ln` → `.mlib` → `.ms` → `package/package.ms`
- local import: `.llib` → `.ln` → `.mlib` → `.ms`

Canonical representations always shadow legacy. The state machine distinguishes
**not found** (may advance to the next permitted representation) from **found but
invalid** (stop — no fallback).

## Preserved compatibility paths (deliberate, not accidental)

- **`.mlib` artifact read** — the reader accepts `MLIB` magic and `.mlib` files.
  A canonical `.llib` missing `AstInterface` is rejected (`InvalidArtifact`), but a
  legacy `.mlib` without `AstInterface` may still reconstruct via `SemanticMetadata`.
- **`.ms` source read** — historical Mellis source extension, probed after `.ln`.
- **`MELLIS_SYSROOT`** — `LUNA_SYSROOT` is preferred; the legacy env var is still read.
- **`emit_mlib` flag + `.mlib` output naming** — explicit legacy compatibility
  emission only. The default/canonical emission (`.llib`, used by `SysrootBuilder`
  and the `luna` CLI) never produces `.mlib`; a legacy `.mlib` is written only when
  `emit_mlib` is set without `emit_llib` (or with an explicit `.mlib` output path).
- **`Mlib*` type names / `MLIB_*` constants** — retained aliases in `luna-llib`
  (no broad rename in this phase).
- **`__mellis_*` runtime ABI** — intentional compatibility surface, not stale naming.

## Frozen invariants

| Invariant | Statement |
| :--- | :--- |
| **COMPAT-CANONICAL-01** | Canonical source is `.ln`; canonical artifact is `.llib` / magic `LLIB`. |
| **COMPAT-WRITE-01** | Default/canonical build output is `.llib` only, and no canonical build path implicitly selects `.mlib`. Legacy `.mlib` emission is possible only on an explicit request. |
| **COMPAT-READ-01** | Legacy formats are read only through explicitly preserved compatibility paths. |
| **COMPAT-PRECEDENCE-01** | Canonical representations take precedence over legacy representations. |
| **COMPAT-INVALID-01** | An existing invalid canonical artifact never falls back to legacy or source. |
| **COMPAT-ABI-01** | Historical runtime ABI names (`__mellis_*`) are intentional compatibility surface. |
| **DEAD-PATH-01** | Every remaining legacy-looking code path has an identified live purpose. |

## Provider identity note

`core/panic` is the canonical logical provider ID (frozen in Phase 4A). The `/` is
part of the logical ID, not a physical path, and there is deliberately no `panic`
alias. This is intentional architecture — do not rename or normalize it.
