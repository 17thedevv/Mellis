pub struct LangContractManifest {
    pub contracts: Vec<LangContractEntry>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisibilityKind {
    /// Items implicitly required by the compiler (e.g., Iterator, Option, Drop)
    CompilerContract,
    /// Ergonomic foundational types/traits added for API parity, not intrinsic to the compiler
    StandardPrelude,
}

pub struct LangContractEntry {
    pub contract_id: &'static str,
    pub provider_id: &'static str,
    pub visibility_kind: VisibilityKind,
    pub auto_visible_symbols: &'static [&'static str],
}

impl LangContractManifest {
    pub fn canonical() -> Self {
        Self {
            contracts: vec![
                // --- Language Contracts ---
                // These are auto-visible because the compiler natively knows about their semantics
                // and expects them in certain language constructs (e.g., `for` loops, `drop`).
                LangContractEntry {
                    contract_id: "drop",
                    provider_id: "__lang_drop",
                    visibility_kind: VisibilityKind::CompilerContract,
                    auto_visible_symbols: &["Drop"],
                },
                LangContractEntry {
                    contract_id: "option",
                    provider_id: "__lang_option",
                    visibility_kind: VisibilityKind::CompilerContract,
                    auto_visible_symbols: &["Option", "Option::Some", "Option::None"],
                },
                LangContractEntry {
                    contract_id: "iterator",
                    provider_id: "__lang_iterator",
                    visibility_kind: VisibilityKind::CompilerContract,
                    auto_visible_symbols: &["Iterator"],
                },
                LangContractEntry {
                    contract_id: "into_iterator",
                    provider_id: "__lang_into_iterator",
                    visibility_kind: VisibilityKind::CompilerContract,
                    auto_visible_symbols: &["IntoIterator"],
                },

                // --- Controlled Standard Prelude ---
                // OptionExt is the only controlled standard-prelude item. Result remains
                // an ordinary explicitly imported core API even though both declarations
                // are supplied by the same logical provider.
                LangContractEntry {
                    contract_id: "result",
                    provider_id: "result",
                    visibility_kind: VisibilityKind::StandardPrelude,
                    auto_visible_symbols: &["OptionExt"],
                },
            ],
        }
    }
}
