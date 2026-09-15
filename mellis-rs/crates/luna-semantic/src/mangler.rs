use luna_common::ids::SymbolId;
use crate::symbol::SymbolTable;
use crate::ty::{BuiltinType, Mutability, SemanticType, SemanticTypeId, TypeContext};

pub struct Mangler;

impl Mangler {
    /// Encode an identifier with its UTF-8 length as prefix: `<len><ident>`.
    pub fn encode_ident(ident: &str) -> String {
        format!("{}{}", ident.len(), ident)
    }

    /// Encode a qualified namespace/module path: `N<ident>+E`.
    pub fn encode_path(segments: &[String]) -> String {
        if segments.is_empty() {
            return "N0E".to_string();
        }
        let mut out = String::from("N");
        for seg in segments {
            out.push_str(&Self::encode_ident(seg));
        }
        out.push('E');
        out
    }

    /// Recursively encode a semantic type into an unambiguous canonical representation.
    pub fn mangle_type(
        types: &TypeContext,
        symbol_table: &SymbolTable,
        ty_id: SemanticTypeId,
    ) -> String {
        let ty_id = types.resolve(ty_id);
        match types.get(ty_id) {
            SemanticType::Void => "v".to_string(),
            SemanticType::Never => "n".to_string(),
            SemanticType::Error => "e".to_string(),
            SemanticType::Primitive(p) => match p {
                BuiltinType::I8 => "a".to_string(),
                BuiltinType::I16 => "s".to_string(),
                BuiltinType::I32 => "l".to_string(),
                BuiltinType::I64 => "x".to_string(),
                BuiltinType::I128 => "w".to_string(),
                BuiltinType::Isize => "i".to_string(),
                BuiltinType::U8 => "h".to_string(),
                BuiltinType::U16 => "t".to_string(),
                BuiltinType::U32 => "m".to_string(),
                BuiltinType::U64 => "y".to_string(),
                BuiltinType::U128 => "o".to_string(),
                BuiltinType::Usize => "j".to_string(),
                BuiltinType::F32 => "f".to_string(),
                BuiltinType::F64 => "d".to_string(),
                BuiltinType::Bool => "b".to_string(),
                BuiltinType::Char => "c".to_string(),
                BuiltinType::String => "z".to_string(),
            },
            SemanticType::Struct(sym_id, generic_args, _) => {
                let full_path = symbol_table.get_full_logical_path(*sym_id);
                let path_enc = Self::encode_path(&full_path);
                if generic_args.is_empty() {
                    format!("T{}E", path_enc)
                } else {
                    let mut args_enc = String::new();
                    for &arg in generic_args {
                        args_enc.push_str(&Self::mangle_type(types, symbol_table, arg));
                    }
                    format!("T{}G{}EE", path_enc, args_enc)
                }
            }
            SemanticType::Enum(sym_id, generic_args, _) => {
                let full_path = symbol_table.get_full_logical_path(*sym_id);
                let path_enc = Self::encode_path(&full_path);
                if generic_args.is_empty() {
                    format!("T{}E", path_enc)
                } else {
                    let mut args_enc = String::new();
                    for &arg in generic_args {
                        args_enc.push_str(&Self::mangle_type(types, symbol_table, arg));
                    }
                    format!("T{}G{}EE", path_enc, args_enc)
                }
            }
            SemanticType::Pointer(mutability, inner) => {
                let m = if *mutability == Mutability::Mutable { "m" } else { "c" };
                format!("P{}{}", m, Self::mangle_type(types, symbol_table, *inner))
            }
            SemanticType::Reference(_, mutability, inner) => {
                let m = if *mutability == Mutability::Mutable { "m" } else { "c" };
                format!("R{}{}", m, Self::mangle_type(types, symbol_table, *inner))
            }
            SemanticType::Array(elem, len) => {
                format!("A{}_{}", len, Self::mangle_type(types, symbol_table, *elem))
            }
            SemanticType::Slice(elem) => {
                format!("S{}", Self::mangle_type(types, symbol_table, *elem))
            }
            SemanticType::Tuple(elems) => {
                let mut elems_enc = String::new();
                for &e in elems {
                    elems_enc.push_str(&Self::mangle_type(types, symbol_table, e));
                }
                format!("U{}E", elems_enc)
            }
            SemanticType::Function { params, return_type } => {
                let mut params_enc = String::new();
                for &p in params {
                    params_enc.push_str(&Self::mangle_type(types, symbol_table, p));
                }
                let ret_enc = Self::mangle_type(types, symbol_table, *return_type);
                format!("W{}E{}", params_enc, ret_enc)
            }
            _ => {
                // Fallback for inference or unresolved vars
                format!("t{}", ty_id.0)
            }
        }
    }

    /// Mangle a free function symbol: `_MF<path>[G<substs>E]`.
    pub fn mangle_function(
        types: &TypeContext,
        symbol_table: &SymbolTable,
        full_path: &[String],
        substs: &[SemanticTypeId],
    ) -> String {
        let path_enc = Self::encode_path(full_path);
        if substs.is_empty() {
            format!("_MF{}", path_enc)
        } else {
            let mut substs_enc = String::new();
            for &s in substs {
                substs_enc.push_str(&Self::mangle_type(types, symbol_table, s));
            }
            format!("_MF{}G{}E", path_enc, substs_enc)
        }
    }

    /// Mangle an inherent method symbol: `_MM<self_path><method_ident>[G<substs>E]`.
    pub fn mangle_method(
        types: &TypeContext,
        symbol_table: &SymbolTable,
        self_path: &[String],
        method_name: &str,
        substs: &[SemanticTypeId],
    ) -> String {
        let self_path_enc = Self::encode_path(self_path);
        let meth_ident = Self::encode_ident(method_name);
        if substs.is_empty() {
            format!("_MM{}{}", self_path_enc, meth_ident)
        } else {
            let mut substs_enc = String::new();
            for &s in substs {
                substs_enc.push_str(&Self::mangle_type(types, symbol_table, s));
            }
            format!("_MM{}{}G{}E", self_path_enc, meth_ident, substs_enc)
        }
    }

    /// Mangle a trait method symbol: `_MI<trait_path>[G<trait_substs>E]<self_ty><method_ident>[G<method_substs>E]`.
    pub fn mangle_trait_method(
        types: &TypeContext,
        symbol_table: &SymbolTable,
        trait_path: &[String],
        trait_substs: &[SemanticTypeId],
        self_ty: SemanticTypeId,
        method_name: &str,
        method_substs: &[SemanticTypeId],
    ) -> String {
        let trait_path_enc = Self::encode_path(trait_path);
        let mut trait_substs_enc = String::new();
        if !trait_substs.is_empty() {
            trait_substs_enc.push('G');
            for &s in trait_substs {
                trait_substs_enc.push_str(&Self::mangle_type(types, symbol_table, s));
            }
            trait_substs_enc.push('E');
        }

        let self_ty_enc = Self::mangle_type(types, symbol_table, self_ty);
        let meth_ident = Self::encode_ident(method_name);

        let mut method_substs_enc = String::new();
        if !method_substs.is_empty() {
            method_substs_enc.push('G');
            for &s in method_substs {
                method_substs_enc.push_str(&Self::mangle_type(types, symbol_table, s));
            }
            method_substs_enc.push('E');
        }

        format!(
            "_MI{}{}{}{}{}",
            trait_path_enc, trait_substs_enc, self_ty_enc, meth_ident, method_substs_enc
        )
    }

    /// Mangle a drop glue symbol: `__mellis_drop_glue_<mangled_concrete_type>`.
    pub fn mangle_drop_glue(
        types: &TypeContext,
        symbol_table: &SymbolTable,
        concrete_ty: SemanticTypeId,
    ) -> String {
        let type_enc = Self::mangle_type(types, symbol_table, concrete_ty);
        format!("__mellis_drop_glue_{}", type_enc)
    }
}
