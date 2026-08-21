---
name: mellis-audit
description: Audit protocol to verify feature completion
---

# Mellis Audit Protocol

This skill does not code features. It only verifies them.

## Workflow

When asked to audit a claim like "Feature X is complete", check the following:
- syntax exists?
- AST exists?
- semantic exists?
- lowering exists?
- backend exists?
- runtime exists?
- positive test?
- negative test?
- cross-module test?
- actual executable verified?

## Output

Report the status strictly as one of the following:
- COMPLETE
- PARTIAL
- UNVERIFIED
- BROKEN

Absolutely DO NOT output "✅ 100%" or equivalent without explicit evidence for all the items in the checklist above.

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
