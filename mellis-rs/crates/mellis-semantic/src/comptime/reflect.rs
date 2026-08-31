use crate::{SemanticContext, ty::{SemanticTypeId, SemanticType, BuiltinType}};

pub struct ComptimeReflection;

impl ComptimeReflection {
    pub fn sizeof(ty_id: SemanticTypeId, ctx: &SemanticContext) -> usize {
        match ctx.types.get(ty_id) {
            SemanticType::Primitive(b) => match b {
                BuiltinType::I8 | BuiltinType::U8 | BuiltinType::Bool => 1,
                BuiltinType::I16 | BuiltinType::U16 => 2,
                BuiltinType::I32 | BuiltinType::U32 | BuiltinType::F32 | BuiltinType::Char => 4,
                BuiltinType::I64 | BuiltinType::U64 | BuiltinType::F64 | BuiltinType::Isize | BuiltinType::Usize => 8,
                BuiltinType::I128 | BuiltinType::U128 => 16,
                BuiltinType::String => 16, // ptr + len
            },
            SemanticType::Pointer(_, _) | SemanticType::Reference(_, _, _) | SemanticType::Box(_) => 8,
            SemanticType::Array(elem, len) => Self::sizeof(*elem, ctx) * (*len as usize),
            SemanticType::Slice(_) => 16, // ptr + len
            SemanticType::Tuple(elems) => {
                let mut size = 0;
                for elem in elems {
                    let elem_align = Self::alignof(*elem, ctx);
                    let padding = (elem_align - (size % elem_align)) % elem_align;
                    size += padding + Self::sizeof(*elem, ctx);
                }
                let tuple_align = Self::alignof(ty_id, ctx);
                if tuple_align > 0 {
                    let padding = (tuple_align - (size % tuple_align)) % tuple_align;
                    size += padding;
                }
                size
            }
            SemanticType::Struct(sym_id, _) => {
                // Approximate struct size based on symbol fields if present
                8
            }
            SemanticType::Enum(_, _) => 8,
            SemanticType::Void | SemanticType::Never => 0,
            _ => 8,
        }
    }

    pub fn alignof(ty_id: SemanticTypeId, ctx: &SemanticContext) -> usize {
        match ctx.types.get(ty_id) {
            SemanticType::Primitive(b) => match b {
                BuiltinType::I8 | BuiltinType::U8 | BuiltinType::Bool => 1,
                BuiltinType::I16 | BuiltinType::U16 => 2,
                BuiltinType::I32 | BuiltinType::U32 | BuiltinType::F32 | BuiltinType::Char => 4,
                BuiltinType::I64 | BuiltinType::U64 | BuiltinType::F64 | BuiltinType::Isize | BuiltinType::Usize => 8,
                BuiltinType::I128 | BuiltinType::U128 => 16,
                BuiltinType::String => 16,
            },
            SemanticType::Pointer(_, _) | SemanticType::Reference(_, _, _) | SemanticType::Box(_) => 8,
            SemanticType::Array(elem, _) => Self::alignof(*elem, ctx),
            SemanticType::Slice(_) => 8,
            SemanticType::Tuple(elems) => {
                let mut max_align = 1;
                for elem in elems {
                    max_align = max_align.max(Self::alignof(*elem, ctx));
                }
                max_align
            }
            _ => 8,
        }
    }

    pub fn type_name(ty_id: SemanticTypeId, ctx: &SemanticContext) -> String {
        match ctx.types.get(ty_id) {
            SemanticType::Primitive(b) => match b {
                BuiltinType::I8 => "i8".to_string(),
                BuiltinType::I16 => "i16".to_string(),
                BuiltinType::I32 => "i32".to_string(),
                BuiltinType::I64 => "i64".to_string(),
                BuiltinType::I128 => "i128".to_string(),
                BuiltinType::Isize => "isize".to_string(),
                BuiltinType::U8 => "u8".to_string(),
                BuiltinType::U16 => "u16".to_string(),
                BuiltinType::U32 => "u32".to_string(),
                BuiltinType::U64 => "u64".to_string(),
                BuiltinType::U128 => "u128".to_string(),
                BuiltinType::Usize => "usize".to_string(),
                BuiltinType::F32 => "f32".to_string(),
                BuiltinType::F64 => "f64".to_string(),
                BuiltinType::Bool => "bool".to_string(),
                BuiltinType::String => "str".to_string(),
                BuiltinType::Char => "char".to_string(),
            },
            SemanticType::Pointer(mutability, inner) => {
                let m = if *mutability == crate::ty::Mutability::Mutable { "*rw " } else { "* " };
                format!("{}{}", m, Self::type_name(*inner, ctx))
            }
            SemanticType::Reference(_, mutability, inner) => {
                let m = if *mutability == crate::ty::Mutability::Mutable { "&rw " } else { "& " };
                format!("{}{}", m, Self::type_name(*inner, ctx))
            }
            SemanticType::Array(inner, len) => format!("[{}; {}]", Self::type_name(*inner, ctx), len),
            SemanticType::Slice(inner) => format!("[{}]", Self::type_name(*inner, ctx)),
            SemanticType::Tuple(elems) => {
                let names: Vec<String> = elems.iter().map(|e| Self::type_name(*e, ctx)).collect();
                format!("({})", names.join(", "))
            }
            SemanticType::Struct(sym_id, _) => ctx.symbol_table.get_symbol(*sym_id).name.clone(),
            SemanticType::Enum(sym_id, _) => ctx.symbol_table.get_symbol(*sym_id).name.clone(),
            SemanticType::Void => "void".to_string(),
            SemanticType::Never => "never".to_string(),
            _ => "<type>".to_string(),
        }
    }
}
