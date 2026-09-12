use luna_common::ids::SymbolId;
use luna_common::diagnostic::Diagnostic;
use luna_common::Span;
use std::collections::HashMap;

/// The kinds of declarations that can be marked with `#[lang]`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LangItemTarget {
    Trait,
    Struct,
    Enum,
    EnumVariant,
    Function,
    Method,
}

macro_rules! lang_item_table {
    (
        $(
            ($variant:ident, $name:expr, $target:ident);
        )*
    ) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        #[repr(usize)]
        pub enum LangItem {
            $(
                $variant,
            )*
        }

        impl LangItem {
            pub const COUNT: usize = [$( LangItem::$variant ),*].len();

            pub fn name(&self) -> &'static str {
                match self {
                    $(
                        LangItem::$variant => $name,
                    )*
                }
            }

            pub fn target(&self) -> LangItemTarget {
                match self {
                    $(
                        LangItem::$variant => LangItemTarget::$target,
                    )*
                }
            }

            pub fn from_name(name: &str) -> Option<Self> {
                match name {
                    $(
                        $name => Some(LangItem::$variant),
                    )*
                    _ => None,
                }
            }

            pub fn as_usize(&self) -> usize {
                *self as usize
            }
        }
    }
}

// 5B-2B will populate this table. For 5B-2A, we just need a dummy to ensure the infrastructure builds,
// or we can just leave it empty if the macro allows empty lists, but Rust arrays need size > 0.
// Let's just define a few placeholders for 5B-2A testing.
lang_item_table! {
    (TestTrait, "test_trait", Trait);
    (TestStruct, "test_struct", Struct);
    (Drop, "drop", Trait);
    (Copy, "copy", Trait);
    (DropFn, "drop_fn", Method);
    (Try, "try", Trait);
    (TryFromOutput, "try_from_output", Method);
    (TryBranch, "try_branch", Method);
    (FromResidual, "from_residual", Trait);
    (FromResidualFn, "from_residual_fn", Method);
    (ControlFlow, "control_flow", Enum);
    (ControlFlowContinue, "continue", EnumVariant);
    (ControlFlowBreak, "break", EnumVariant);
}

#[derive(Clone, Debug)]
pub struct LangItemRegistry {
    items: [Option<SymbolId>; LangItem::COUNT],
    reverse: HashMap<SymbolId, LangItem>,
}

impl LangItemRegistry {
    pub fn new() -> Self {
        Self {
            items: [None; LangItem::COUNT],
            reverse: HashMap::new(),
        }
    }

    pub fn get(&self, item: LangItem) -> Option<SymbolId> {
        self.items[item.as_usize()]
    }

    pub fn require(&self, item: LangItem, span: Span, diagnostics: &mut Vec<Diagnostic>) -> Result<SymbolId, ()> {
        if let Some(sym) = self.items[item.as_usize()] {
            Ok(sym)
        } else {
            diagnostics.push(Diagnostic::error(format!("language item `{}` is required, but it is not defined", item.name())).with_span(span));
            Err(())
        }
    }

    
    pub fn iter(&self) -> impl Iterator<Item = (LangItem, SymbolId)> + '_ {
        self.reverse.iter().map(|(&sym, &item)| (item, sym))
    }

    pub fn inject_raw(&mut self, item: LangItem, sym: SymbolId) {
        if self.items[item.as_usize()].is_none() {
            self.items[item.as_usize()] = Some(sym);
            self.reverse.insert(sym, item);
        }
    }

    pub fn from_symbol(&self, sym: SymbolId) -> Option<LangItem> {
        self.reverse.get(&sym).copied()
    }

    pub fn register(&mut self, item: LangItem, sym: SymbolId, actual_target: LangItemTarget, span: Span, diagnostics: &mut Vec<Diagnostic>) {
        let expected_target = item.target();
        if actual_target != expected_target {
            diagnostics.push(Diagnostic::error(format!("language item `{}` requires target {:?}, but found {:?}", item.name(), expected_target, actual_target)).with_span(span));
            return;
        }

        if let Some(&existing_item) = self.reverse.get(&sym) {
            diagnostics.push(Diagnostic::error(format!("symbol is already registered as language item `{}`", existing_item.name())).with_span(span));
            return;
        }

        if let Some(_old_sym) = self.items[item.as_usize()] {
            diagnostics.push(Diagnostic::error(format!("language item `{}` is defined multiple times", item.name())).with_span(span));
            return;
        }

        self.items[item.as_usize()] = Some(sym);
        self.reverse.insert(sym, item);
    }

    pub fn inject_from(&mut self, other: &LangItemRegistry, symbol_map: &HashMap<SymbolId, SymbolId>) {
        for i in 0..LangItem::COUNT {
            if let Some(old_sym) = other.items[i] {
                if let Some(&new_sym) = symbol_map.get(&old_sym) {
                    if self.items[i].is_none() {
                        self.items[i] = Some(new_sym);
                        // We need the enum variant to put into reverse map!
                        // But we don't have it easily from index.
                        // However, other.reverse has old_sym -> item!
                        if let Some(&item) = other.reverse.get(&old_sym) {
                            self.reverse.insert(new_sym, item);
                        }
                    }
                }
            }
        }
    }
}
