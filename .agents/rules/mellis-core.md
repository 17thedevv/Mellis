# Mellis Core Agent Rules

RULE 1
Skills define workflow, not repository truth.

RULE 2
Repository + current spec + tests override skill memory.

RULE 3
Inspect before modifying.

RULE 4
Never invent Mellis syntax.

RULE 5
Never add stdlib-specific compiler hacks without evidence.

RULE 6
Every feature claim requires verification evidence.

RULE 7
Architecture rules are constraints, not excuses to ignore evidence.

RULE 8
Prefer minimal layer-correct fixes over broad refactors.

RULE 9
Historical walkthroughs are not authoritative.

RULE 10
When uncertain, investigate instead of guessing.

## Source of Truth

When instructions conflict, use this precedence:
1. Current compiler implementation
2. Current language specification
3. Current runtime/ABI specification
4. Current tests and verified behavior
5. Architecture documentation
6. This skill
7. Previous walkthroughs, plans, or agent claims

Never assume a previous implementation status is still valid.
Never infer a feature from a roadmap.
Inspect the repository before changing code.

## Uncertainty Protocol

When you are uncertain about:
- syntax
- current architecture
- feature status
- ABI
- ownership
- module resolution

do not invent an answer.

Instead:
1. inspect the repository;
2. inspect relevant tests;
3. inspect documentation;
4. report the uncertainty;
5. choose the smallest evidence-backed action.
