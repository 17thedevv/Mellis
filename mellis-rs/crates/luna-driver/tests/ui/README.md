# Mellis E2E UI Test Suite

## Porting Decisions and MVIR Test Coverage

This suite validates the end-to-end functionality of the Mellis compiler by passing `.ms` files through the entire pipeline: `Parser -> Semantic -> MVIR -> Borrow Checker`.

**Important context on coverage:**
Prior to this UI suite, 31 tests in `crates/mellis-borrowck/tests/` tested the Borrow Checker by directly hardcoding MVIR and skipping the frontend (parser & semantic). The tests in this directory are the *End-to-End representative port* of those 31 MVIR tests.

We intentionally ported a representative subset that covers all main invariants (mutability, move-after-use, non-lexical lifetimes, alias conflicts, exhaustiveness, FFI callbacks) instead of porting every single variation. The remaining un-ported tests in the MVIR-hardcoded suite are still executed as unit tests, and they are considered verified E2E because they share identical code paths with these ported representative tests. 

*If any new safety features are added, an E2E test must be added here; bypassing the frontend for borrowck tests is insufficient on its own.*
