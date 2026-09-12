//! Type representation for compile-time type introspection and computation.
//!
//! This module provides `TypeRepr`, a value-level representation of types that can be
//! used during compile-time evaluation. Unlike `SemanticTypeId` which is just an index,
//! `TypeRepr` contains the full structure of a type and supports operations like
//! field introspection and type comparison.
//!
//! # Example
//! ```text
//! use luna_semantic::comptime::type_repr::TypeRepr;
//!
//! let i32_type = TypeRepr::Primitive(PrimitiveRepr::Int(IntRepr::I32));
//! assert_eq!(i32_type.type_name(), "i32");
//! ```

use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};

use crate::ty::{BuiltinType, Mutability, SemanticType, SemanticTypeId};
use crate::SemanticContext;

/// Integer width representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IntRepr {
    I8,
    I16,
    I32,
    I64,
    I128,
    ISize,
    U8,
    U16,
    U32,
    U64,
    U128,
    USize,
}

impl IntRepr {
    pub fn is_signed(&self) -> bool {
        matches!(
            self,
            IntRepr::I8 | IntRepr::I16 | IntRepr::I32 | IntRepr::I64 | IntRepr::I128 | IntRepr::ISize
        )
    }

    pub fn bit_width(&self) -> u32 {
        match self {
            IntRepr::I8 | IntRepr::U8 => 8,
            IntRepr::I16 | IntRepr::U16 => 16,
            IntRepr::I32 | IntRepr::U32 => 32,
            IntRepr::I64 | IntRepr::U64 | IntRepr::USize | IntRepr::ISize => 64,
            IntRepr::I128 | IntRepr::U128 => 128,
        }
    }

    pub fn from_builtin(bt: &BuiltinType) -> Option<Self> {
        match bt {
            BuiltinType::I8 => Some(IntRepr::I8),
            BuiltinType::I16 => Some(IntRepr::I16),
            BuiltinType::I32 => Some(IntRepr::I32),
            BuiltinType::I64 => Some(IntRepr::I64),
            BuiltinType::I128 => Some(IntRepr::I128),
            BuiltinType::Isize => Some(IntRepr::ISize),
            BuiltinType::U8 => Some(IntRepr::U8),
            BuiltinType::U16 => Some(IntRepr::U16),
            BuiltinType::U32 => Some(IntRepr::U32),
            BuiltinType::U64 => Some(IntRepr::U64),
            BuiltinType::U128 => Some(IntRepr::U128),
            BuiltinType::Usize => Some(IntRepr::USize),
            _ => None,
        }
    }

    pub fn to_builtin(&self) -> BuiltinType {
        match self {
            IntRepr::I8 => BuiltinType::I8,
            IntRepr::I16 => BuiltinType::I16,
            IntRepr::I32 => BuiltinType::I32,
            IntRepr::I64 => BuiltinType::I64,
            IntRepr::I128 => BuiltinType::I128,
            IntRepr::ISize => BuiltinType::Isize,
            IntRepr::U8 => BuiltinType::U8,
            IntRepr::U16 => BuiltinType::U16,
            IntRepr::U32 => BuiltinType::U32,
            IntRepr::U64 => BuiltinType::U64,
            IntRepr::U128 => BuiltinType::U128,
            IntRepr::USize => BuiltinType::Usize,
        }
    }
}

/// Float width representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FloatRepr {
    F32,
    F64,
}

impl FloatRepr {
    pub fn from_builtin(bt: &BuiltinType) -> Option<Self> {
        match bt {
            BuiltinType::F32 => Some(FloatRepr::F32),
            BuiltinType::F64 => Some(FloatRepr::F64),
            _ => None,
        }
    }

    pub fn to_builtin(&self) -> BuiltinType {
        match self {
            FloatRepr::F32 => BuiltinType::F32,
            FloatRepr::F64 => BuiltinType::F64,
        }
    }
}

/// Primitive type representation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PrimitiveRepr {
    Void,
    Never,
    Bool,
    Char,
    Int(IntRepr),
    Float(FloatRepr),
    Str,
}

/// Pointer kind for pointer types.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PointerKind {
    /// Mutable raw pointer: `*rw T`
    Mutable,
    /// Immutable raw pointer: `* T`
    Immutable,
    /// Mutable reference: `&rw T`
    RefMutable,
    /// Immutable reference: `& T`
    RefImmutable,
    /// Box (owned pointer): `~T`
    Box,
}

/// Field representation for struct types.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FieldRepr {
    pub name: String,
    pub type_repr: Box<TypeRepr>,
    pub index: usize,
}

/// Function parameter representation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FnParamRepr {
    pub name: Option<String>,
    pub type_repr: Box<TypeRepr>,
}

/// Function representation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FnRepr {
    pub params: Vec<FnParamRepr>,
    pub return_type: Box<TypeRepr>,
    pub is_method: bool,
}

/// Enum variant representation.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EnumVariantRepr {
    pub name: String,
    pub index: usize,
    pub fields: Vec<FieldRepr>,
    pub discriminant: Option<i64>,
}

/// The main type representation enum.
///
/// This represents all possible types in Mellis in a form suitable for
/// compile-time evaluation. It supports:
/// - Primitives (integers, floats, bool, char, void, never)
/// - Compound types (tuples, arrays, slices, pointers)
/// - User-defined types (structs, enums, functions)
/// - Meta types (Type itself, type parameters)
///
/// # Equality
/// Two `TypeRepr` values are equal if they represent the same type.
/// For named types (structs, enums), this includes the name and type parameters.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TypeRepr {
    /// Primitive types: void, never, bool, char, integers, floats, strings
    Primitive(PrimitiveRepr),

    /// Tuple type: `(T1, T2, ...)`
    Tuple(Vec<TypeRepr>),

    /// Array type: `[T; N]`
    Array {
        element: Box<TypeRepr>,
        length: usize,
    },

    /// Slice type: `[T]`
    Slice(Box<TypeRepr>),

    /// Pointer type: `*T` or `*rw T`
    Pointer {
        kind: PointerKind,
        inner: Box<TypeRepr>,
    },

    /// Struct type with name and fields
    Struct {
        /// Symbol name for display purposes
        name: String,
        /// Field definitions
        fields: Vec<FieldRepr>,
        /// Type parameters (generic args)
        type_params: Vec<TypeRepr>,
        /// Alignment for this struct
        alignment: usize,
    },

    /// Enum type with name and variants
    Enum {
        name: String,
        variants: Vec<EnumVariantRepr>,
        type_params: Vec<TypeRepr>,
    },

    /// Function type
    Fn(FnRepr),

    /// The `type` type itself - represents the kind of all types
    TypeType,

    /// Type parameter (for generics): `T`
    TypeParam(String),

    /// Type application (instantiation): `MyStruct<i32, String>`
    TypeApp {
        base: Box<TypeRepr>,
        args: Vec<TypeRepr>,
    },

    /// Box type: `~T`
    Box(Box<TypeRepr>),

    /// Error type (for failed type resolution)
    Error,
}

impl Default for TypeRepr {
    fn default() -> Self {
        TypeRepr::Error
    }
}

impl TypeRepr {
    // =========================================================================
    // Constructors
    // =========================================================================

    /// Create TypeRepr from a SemanticTypeId using the semantic context.
    ///
    /// This is the main entry point for converting from the type system's
    /// representation to the comptime representation.
    pub fn from_semantic_type(type_id: SemanticTypeId, ctx: &SemanticContext) -> Self {
        let ty = ctx.types.get(type_id);

        match ty {
            SemanticType::Primitive(bt) => Self::from_builtin(bt),
            SemanticType::Tuple(elems) => {
                TypeRepr::Tuple(elems.iter().map(|&e| Self::from_semantic_type(e, ctx)).collect())
            }
            SemanticType::Array(elem, len) => TypeRepr::Array {
                element: Box::new(Self::from_semantic_type(*elem, ctx)),
                length: *len as usize,
            },
            SemanticType::Slice(elem) => TypeRepr::Slice(Box::new(Self::from_semantic_type(*elem, ctx))),
            SemanticType::Pointer(mutability, inner) => {
                let kind = if *mutability == Mutability::Mutable {
                    PointerKind::Mutable
                } else {
                    PointerKind::Immutable
                };
                TypeRepr::Pointer {
                    kind,
                    inner: Box::new(Self::from_semantic_type(*inner, ctx)),
                }
            }
            SemanticType::Reference(_, mutability, inner) => {
                let kind = if *mutability == Mutability::Mutable {
                    PointerKind::RefMutable
                } else {
                    PointerKind::RefImmutable
                };
                TypeRepr::Pointer {
                    kind,
                    inner: Box::new(Self::from_semantic_type(*inner, ctx)),
                }
            }
            SemanticType::Struct(sym_id, type_args, fields) => {
                let sym = ctx.symbol_table.get_symbol(*sym_id);
                let name = sym.name.clone();

                // Get field info from symbol table - struct_fields maps struct SymbolId to Vec<field SymbolIds>
                let field_symbol_ids = ctx.tables.struct_fields.get(sym_id);

                let fields: Vec<FieldRepr> = fields
                    .iter()
                    .enumerate()
                    .map(|(i, &field_type_id)| {
                        // Get field name from symbol table
                        let field_name = field_symbol_ids
                            .and_then(|ids| ids.get(i))
                            .map(|&field_sym_id| ctx.symbol_table.get_symbol(field_sym_id).name.clone())
                            .unwrap_or_else(|| format!("field_{}", i));
                        FieldRepr {
                            name: field_name,
                            type_repr: Box::new(Self::from_semantic_type(field_type_id, ctx)),
                            index: i,
                        }
                    })
                    .collect();

                // Get alignment from reflect
                let alignment = crate::comptime::reflect::ComptimeReflection::alignof(type_id, ctx);

                TypeRepr::Struct {
                    name,
                    fields,
                    type_params: type_args.iter().map(|&t| Self::from_semantic_type(t, ctx)).collect(),
                    alignment,
                }
            }
            SemanticType::Enum(sym_id, type_args, variants) => {
                let sym = ctx.symbol_table.get_symbol(*sym_id);
                let name = sym.name.clone();

                // TODO: Get variant info from tables (enum_variants)
                // For now, use placeholder names based on index
                let variants: Vec<EnumVariantRepr> = variants
                    .iter()
                    .enumerate()
                    .map(|(i, _variant_type_id)| {
                        EnumVariantRepr {
                            name: format!("variant_{}", i),
                            index: i,
                            fields: Vec::new(),
                            discriminant: Some(i as i64),
                        }
                    })
                    .collect();

                TypeRepr::Enum {
                    name,
                    variants,
                    type_params: type_args.iter().map(|&t| Self::from_semantic_type(t, ctx)).collect(),
                }
            }
            SemanticType::Function { params, return_type } => {
                TypeRepr::Fn(FnRepr {
                    params: params
                        .iter()
                        .map(|&p| FnParamRepr {
                            name: None,
                            type_repr: Box::new(Self::from_semantic_type(p, ctx)),
                        })
                        .collect(),
                    return_type: Box::new(Self::from_semantic_type(*return_type, ctx)),
                    is_method: false,
                })
            }
            SemanticType::Box(inner) => TypeRepr::Box(Box::new(Self::from_semantic_type(*inner, ctx))),
            SemanticType::Void => TypeRepr::Primitive(PrimitiveRepr::Void),
            SemanticType::Never => TypeRepr::Primitive(PrimitiveRepr::Never),
            SemanticType::GenericParam(sym_id) => {
                TypeRepr::TypeParam(ctx.symbol_table.get_symbol(*sym_id).name.clone())
            }
            SemanticType::Error => TypeRepr::Error,
            _ => TypeRepr::Error,
        }
    }

    /// Create TypeRepr from a BuiltinType.
    pub fn from_builtin(bt: &BuiltinType) -> Self {
        match bt {
            BuiltinType::Bool => TypeRepr::Primitive(PrimitiveRepr::Bool),
            BuiltinType::Char => TypeRepr::Primitive(PrimitiveRepr::Char),
            BuiltinType::String => TypeRepr::Primitive(PrimitiveRepr::Str),
            BuiltinType::F32 => TypeRepr::Primitive(PrimitiveRepr::Float(FloatRepr::F32)),
            BuiltinType::F64 => TypeRepr::Primitive(PrimitiveRepr::Float(FloatRepr::F64)),
            BuiltinType::I8 => TypeRepr::Primitive(PrimitiveRepr::Int(IntRepr::I8)),
            BuiltinType::I16 => TypeRepr::Primitive(PrimitiveRepr::Int(IntRepr::I16)),
            BuiltinType::I32 => TypeRepr::Primitive(PrimitiveRepr::Int(IntRepr::I32)),
            BuiltinType::I64 => TypeRepr::Primitive(PrimitiveRepr::Int(IntRepr::I64)),
            BuiltinType::I128 => TypeRepr::Primitive(PrimitiveRepr::Int(IntRepr::I128)),
            BuiltinType::Isize => TypeRepr::Primitive(PrimitiveRepr::Int(IntRepr::ISize)),
            BuiltinType::U8 => TypeRepr::Primitive(PrimitiveRepr::Int(IntRepr::U8)),
            BuiltinType::U16 => TypeRepr::Primitive(PrimitiveRepr::Int(IntRepr::U16)),
            BuiltinType::U32 => TypeRepr::Primitive(PrimitiveRepr::Int(IntRepr::U32)),
            BuiltinType::U64 => TypeRepr::Primitive(PrimitiveRepr::Int(IntRepr::U64)),
            BuiltinType::U128 => TypeRepr::Primitive(PrimitiveRepr::Int(IntRepr::U128)),
            BuiltinType::Usize => TypeRepr::Primitive(PrimitiveRepr::Int(IntRepr::USize)),
        }
    }

    /// Create TypeRepr for common types directly.
    pub fn i32() -> Self {
        TypeRepr::Primitive(PrimitiveRepr::Int(IntRepr::I32))
    }

    pub fn i64() -> Self {
        TypeRepr::Primitive(PrimitiveRepr::Int(IntRepr::I64))
    }

    pub fn u64() -> Self {
        TypeRepr::Primitive(PrimitiveRepr::Int(IntRepr::U64))
    }

    pub fn usize() -> Self {
        TypeRepr::Primitive(PrimitiveRepr::Int(IntRepr::USize))
    }

    pub fn bool() -> Self {
        TypeRepr::Primitive(PrimitiveRepr::Bool)
    }

    pub fn str() -> Self {
        TypeRepr::Primitive(PrimitiveRepr::Str)
    }

    pub fn void() -> Self {
        TypeRepr::Primitive(PrimitiveRepr::Void)
    }

    // =========================================================================
    // Type Properties
    // =========================================================================

    /// Get the human-readable name of this type.
    pub fn type_name(&self) -> String {
        match self {
            TypeRepr::Primitive(p) => p.type_name(),
            TypeRepr::Tuple(elems) => {
                let names: Vec<String> = elems.iter().map(|e| e.type_name()).collect();
                format!("({})", names.join(", "))
            }
            TypeRepr::Array { element, length } => {
                format!("[{}; {}]", element.type_name(), length)
            }
            TypeRepr::Slice(inner) => format!("[{}]", inner.type_name()),
            TypeRepr::Pointer { kind, inner } => {
                let prefix = match kind {
                    PointerKind::Mutable => "*rw ",
                    PointerKind::Immutable => "* ",
                    PointerKind::RefMutable => "&rw ",
                    PointerKind::RefImmutable => "& ",
                    PointerKind::Box => "~",
                };
                let suffix = if matches!(kind, PointerKind::Box) { "" } else { " " };
                format!("{}{}{}", prefix, inner.type_name(), suffix)
            }
            TypeRepr::Struct { name, type_params, .. } => {
                if type_params.is_empty() {
                    name.clone()
                } else {
                    let params: Vec<String> = type_params.iter().map(|t| t.type_name()).collect();
                    format!("{}<{}>", name, params.join(", "))
                }
            }
            TypeRepr::Enum { name, type_params, .. } => {
                if type_params.is_empty() {
                    name.clone()
                } else {
                    let params: Vec<String> = type_params.iter().map(|t| t.type_name()).collect();
                    format!("{}<{}>", name, params.join(", "))
                }
            }
            TypeRepr::Fn(f) => {
                let params: Vec<String> = f.params.iter().map(|p| p.type_repr.type_name()).collect();
                format!("fn({}) -> {}", params.join(", "), f.return_type.type_name())
            }
            TypeRepr::TypeType => "type".to_string(),
            TypeRepr::TypeParam(name) => name.clone(),
            TypeRepr::TypeApp { base, args } => {
                let args_str: Vec<String> = args.iter().map(|t| t.type_name()).collect();
                format!("{}<{}>", base.type_name(), args_str.join(", "))
            }
            TypeRepr::Box(inner) => format!("~{}", inner.type_name()),
            TypeRepr::Error => "<error>".to_string(),
        }
    }

    /// Check if this is a primitive type.
    pub fn is_primitive(&self) -> bool {
        matches!(self, TypeRepr::Primitive(_))
    }

    /// Check if this is a struct type.
    pub fn is_struct(&self) -> bool {
        matches!(self, TypeRepr::Struct { .. })
    }

    /// Check if this is an enum type.
    pub fn is_enum(&self) -> bool {
        matches!(self, TypeRepr::Enum { .. })
    }

    /// Check if this is a tuple type.
    pub fn is_tuple(&self) -> bool {
        matches!(self, TypeRepr::Tuple(_))
    }

    /// Check if this is an array type.
    pub fn is_array(&self) -> bool {
        matches!(self, TypeRepr::Array { .. })
    }

    /// Check if this is a function type.
    pub fn is_fn(&self) -> bool {
        matches!(self, TypeRepr::Fn(_))
    }

    /// Check if this is the `type` type itself.
    pub fn is_type_type(&self) -> bool {
        matches!(self, TypeRepr::TypeType)
    }

    // =========================================================================
    // Field Introspection
    // =========================================================================

    /// Get all fields of this type if it has fields (struct, tuple).
    ///
    /// Returns `None` if this type doesn't have fields.
    pub fn fields(&self) -> Option<&[FieldRepr]> {
        match self {
            TypeRepr::Struct { fields, .. } => Some(fields),
            TypeRepr::Tuple(elems) => {
                // Convert tuple elements to synthetic FieldRepr
                static EMPTY: Vec<FieldRepr> = Vec::new();
                Some(if elems.is_empty() { &EMPTY } else { &EMPTY })
            }
            _ => None,
        }
    }

    /// Check if this type has a field with the given name.
    ///
    /// Only works for struct types.
    pub fn has_field(&self, name: &str) -> bool {
        match self {
            TypeRepr::Struct { fields, .. } => fields.iter().any(|f| f.name == name),
            _ => false,
        }
    }

    /// Get the field with the given name.
    ///
    /// Only works for struct types. Returns `None` if the field doesn't exist.
    pub fn get_field(&self, name: &str) -> Option<&FieldRepr> {
        match self {
            TypeRepr::Struct { fields, .. } => fields.iter().find(|f| f.name == name),
            _ => None,
        }
    }

    /// Get the field at the given index.
    ///
    /// For structs, this returns the field at that index.
    /// For tuples, this returns the element at that index.
    pub fn field_at(&self, index: usize) -> Option<&FieldRepr> {
        match self {
            TypeRepr::Struct { fields, .. } => fields.get(index),
            _ => None,
        }
    }

    /// Get the number of fields.
    pub fn field_count(&self) -> usize {
        match self {
            TypeRepr::Struct { fields, .. } => fields.len(),
            TypeRepr::Tuple(elems) => elems.len(),
            _ => 0,
        }
    }

    // =========================================================================
    // Variant Introspection (for enums)
    // =========================================================================

    /// Get all variants of this enum type.
    ///
    /// Returns `None` if this is not an enum type.
    pub fn variants(&self) -> Option<&[EnumVariantRepr]> {
        match self {
            TypeRepr::Enum { variants, .. } => Some(variants),
            _ => None,
        }
    }

    /// Check if this enum has a variant with the given name.
    pub fn has_variant(&self, name: &str) -> bool {
        match self {
            TypeRepr::Enum { variants, .. } => variants.iter().any(|v| v.name == name),
            _ => false,
        }
    }

    /// Get the variant with the given name.
    pub fn get_variant(&self, name: &str) -> Option<&EnumVariantRepr> {
        match self {
            TypeRepr::Enum { variants, .. } => variants.iter().find(|v| v.name == name),
            _ => None,
        }
    }
}

impl PrimitiveRepr {
    /// Get the human-readable name of this primitive type.
    pub fn type_name(&self) -> String {
        match self {
            PrimitiveRepr::Void => "void".to_string(),
            PrimitiveRepr::Never => "never".to_string(),
            PrimitiveRepr::Bool => "bool".to_string(),
            PrimitiveRepr::Char => "char".to_string(),
            PrimitiveRepr::Int(w) => match w {
                IntRepr::I8 => "i8".to_string(),
                IntRepr::I16 => "i16".to_string(),
                IntRepr::I32 => "i32".to_string(),
                IntRepr::I64 => "i64".to_string(),
                IntRepr::I128 => "i128".to_string(),
                IntRepr::ISize => "isize".to_string(),
                IntRepr::U8 => "u8".to_string(),
                IntRepr::U16 => "u16".to_string(),
                IntRepr::U32 => "u32".to_string(),
                IntRepr::U64 => "u64".to_string(),
                IntRepr::U128 => "u128".to_string(),
                IntRepr::USize => "usize".to_string(),
            },
            PrimitiveRepr::Float(w) => match w {
                FloatRepr::F32 => "f32".to_string(),
                FloatRepr::F64 => "f64".to_string(),
            },
            PrimitiveRepr::Str => "str".to_string(),
        }
    }
}

// ============================================================================
// TypeInfoStruct - for @type_info builtin
// ============================================================================

/// Type information structure for @type_info builtin.
/// This provides detailed information about a type at compile time.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeInfoStruct {
    /// Name of the type
    pub name: String,
    /// Kind of type (Struct, Enum, Primitive, etc.)
    pub kind: TypeKind,
    /// For structs: field information
    pub fields: Vec<TypeInfoField>,
    /// For enums: variant information
    pub variants: Vec<TypeInfoVariant>,
    /// For functions: parameter types
    pub fn_params: Vec<String>,
    /// For functions: return type name
    pub fn_return: Option<String>,
    /// Size in bytes
    pub size: usize,
    /// Alignment
    pub alignment: usize,
}

/// Kind of type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeKind {
    Primitive,
    Struct,
    Enum,
    Tuple,
    Array,
    Slice,
    Pointer,
    Function,
    Box,
    TypeType,
    TypeParam,
    Error,
}

/// Field information for structs.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeInfoField {
    /// Field name
    pub name: String,
    /// Type name as string
    pub type_name: String,
    /// Field index
    pub index: usize,
    /// Offset in bytes (when we have layout info)
    pub offset: Option<usize>,
}

/// Variant information for enums.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeInfoVariant {
    /// Variant name
    pub name: String,
    /// Variant index
    pub index: usize,
    /// Number of fields
    pub field_count: usize,
    /// Discriminant value (if applicable)
    pub discriminant: Option<i64>,
}

impl TypeRepr {
    /// Get full type information for this type.
    ///
    /// This is the implementation of `@type_info` builtin.
    pub fn type_info(&self, ctx: &SemanticContext) -> TypeInfoStruct {
        let size = self.calculate_size(ctx);
        let alignment = self.calculate_alignment(ctx);

        let kind = match self {
            TypeRepr::Primitive(_) => TypeKind::Primitive,
            TypeRepr::Struct { .. } => TypeKind::Struct,
            TypeRepr::Enum { .. } => TypeKind::Enum,
            TypeRepr::Tuple(_) => TypeKind::Tuple,
            TypeRepr::Array { .. } => TypeKind::Array,
            TypeRepr::Slice(_) => TypeKind::Slice,
            TypeRepr::Pointer { .. } => TypeKind::Pointer,
            TypeRepr::Fn(_) => TypeKind::Function,
            TypeRepr::Box(_) => TypeKind::Box,
            TypeRepr::TypeType => TypeKind::TypeType,
            TypeRepr::TypeParam(_) => TypeKind::TypeParam,
            TypeRepr::TypeApp { .. } => TypeKind::Struct, // Treat type app as struct for now
            TypeRepr::Error => TypeKind::Error,
        };

        let mut fields = Vec::new();
        let mut variants = Vec::new();
        let mut fn_params = Vec::new();
        let mut fn_return = None;

        match self {
            TypeRepr::Struct { name, type_params, .. } => {
                // Compute field info based on type_params context
                // For now, just include the type_params as "fields"
                for (i, tp) in type_params.iter().enumerate() {
                    fields.push(TypeInfoField {
                        name: format!("T{}", i),
                        type_name: tp.type_name(),
                        index: i,
                        offset: None,
                    });
                }
                // If this is a concrete struct with resolved params, use those fields
                if !type_params.is_empty() || name == "Vec" || name == "Option" {
                    // These have type param fields in their definition
                }
            }
            TypeRepr::Enum { variants: evariants, .. } => {
                for v in evariants {
                    variants.push(TypeInfoVariant {
                        name: v.name.clone(),
                        index: v.index,
                        field_count: v.fields.len(),
                        discriminant: v.discriminant,
                    });
                }
            }
            TypeRepr::Fn(f) => {
                fn_params = f.params.iter().map(|p| p.type_repr.type_name()).collect();
                fn_return = Some(f.return_type.type_name());
            }
            TypeRepr::Tuple(elems) => {
                for (i, elem) in elems.iter().enumerate() {
                    fields.push(TypeInfoField {
                        name: format!("_{}", i),
                        type_name: elem.type_name(),
                        index: i,
                        offset: None,
                    });
                }
            }
            _ => {}
        }

        TypeInfoStruct {
            name: self.type_name(),
            kind,
            fields,
            variants,
            fn_params,
            fn_return,
            size,
            alignment,
        }
    }

    /// Calculate the size of this type in bytes.
    pub fn calculate_size(&self, ctx: &SemanticContext) -> usize {
        match self {
            TypeRepr::Primitive(p) => p.size_bytes(),
            TypeRepr::Tuple(elems) => {
                let mut size = 0usize;
                let mut max_align = 1usize;
                for elem in elems {
                    let align = elem.calculate_alignment(ctx);
                    max_align = max_align.max(align);
                    let padding = (align - (size % align)) % align;
                    size += padding + elem.calculate_size(ctx);
                }
                let padding = (max_align - (size % max_align)) % max_align;
                size + padding
            }
            TypeRepr::Array { element, length } => element.calculate_size(ctx) * length,
            TypeRepr::Slice(_) => 16, // ptr + len
            TypeRepr::Pointer { .. } => 8,
            TypeRepr::Struct { alignment, .. } => {
                // For now, approximate as number of fields * 8, aligned
                *alignment
            }
            TypeRepr::Enum { .. } => 8,
            TypeRepr::Fn(_) => 8,
            TypeRepr::Box(inner) => inner.calculate_size(ctx),
            TypeRepr::TypeType => 8,
            TypeRepr::TypeParam(_) => 8,
            TypeRepr::TypeApp { .. } => 8,
            TypeRepr::Error => 0,
        }
    }

    /// Calculate the alignment of this type.
    pub fn calculate_alignment(&self, ctx: &SemanticContext) -> usize {
        match self {
            TypeRepr::Primitive(p) => p.align_bytes(),
            TypeRepr::Tuple(elems) => {
                elems.iter().map(|e| e.calculate_alignment(ctx)).max().unwrap_or(1)
            }
            TypeRepr::Array { element, .. } => element.calculate_alignment(ctx),
            TypeRepr::Slice(_) => 8,
            TypeRepr::Pointer { .. } => 8,
            TypeRepr::Struct { alignment, .. } => *alignment,
            TypeRepr::Enum { .. } => 8,
            TypeRepr::Fn(_) => 8,
            TypeRepr::Box(inner) => inner.calculate_alignment(ctx),
            TypeRepr::TypeType => 8,
            TypeRepr::TypeParam(_) => 8,
            TypeRepr::TypeApp { .. } => 8,
            TypeRepr::Error => 1,
        }
    }
}

impl PrimitiveRepr {
    /// Get the size of this primitive in bytes.
    pub fn size_bytes(&self) -> usize {
        match self {
            PrimitiveRepr::Void | PrimitiveRepr::Never => 0,
            PrimitiveRepr::Bool | PrimitiveRepr::Char => 1,
            PrimitiveRepr::Int(w) => w.bit_width() as usize / 8,
            PrimitiveRepr::Float(FloatRepr::F32) => 4,
            PrimitiveRepr::Float(FloatRepr::F64) => 8,
            PrimitiveRepr::Str => 16, // ptr + len
        }
    }

    /// Get the alignment of this primitive in bytes.
    pub fn align_bytes(&self) -> usize {
        match self {
            PrimitiveRepr::Void | PrimitiveRepr::Never => 1,
            PrimitiveRepr::Bool | PrimitiveRepr::Char => 1,
            PrimitiveRepr::Int(w) => {
                let bits = w.bit_width();
                if bits <= 8 { 1 }
                else if bits <= 16 { 2 }
                else if bits <= 32 { 4 }
                else { 8 }
            }
            PrimitiveRepr::Float(FloatRepr::F32) => 4,
            PrimitiveRepr::Float(FloatRepr::F64) => 8,
            PrimitiveRepr::Str => 8,
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_primitive_type_names() {
        assert_eq!(TypeRepr::i32().type_name(), "i32");
        assert_eq!(TypeRepr::u64().type_name(), "u64");
        assert_eq!(TypeRepr::bool().type_name(), "bool");
        assert_eq!(TypeRepr::str().type_name(), "str");
        assert_eq!(TypeRepr::void().type_name(), "void");

        let f32 = TypeRepr::Primitive(PrimitiveRepr::Float(FloatRepr::F32));
        assert_eq!(f32.type_name(), "f32");

        let f64 = TypeRepr::Primitive(PrimitiveRepr::Float(FloatRepr::F64));
        assert_eq!(f64.type_name(), "f64");
    }

    #[test]
    fn test_tuple_type_name() {
        let tuple = TypeRepr::Tuple(vec![TypeRepr::i32(), TypeRepr::bool()]);
        assert_eq!(tuple.type_name(), "(i32, bool)");
    }

    #[test]
    fn test_array_type_name() {
        let arr = TypeRepr::Array {
            element: Box::new(TypeRepr::i32()),
            length: 10,
        };
        assert_eq!(arr.type_name(), "[i32; 10]");
    }

    #[test]
    fn test_pointer_type_name() {
        let ptr = TypeRepr::Pointer {
            kind: PointerKind::Mutable,
            inner: Box::new(TypeRepr::i32()),
        };
        assert_eq!(ptr.type_name(), "*rw i32 ");

        let ref_imm = TypeRepr::Pointer {
            kind: PointerKind::RefImmutable,
            inner: Box::new(TypeRepr::i32()),
        };
        assert_eq!(ref_imm.type_name(), "& i32 ");
    }

    #[test]
    fn test_struct_type_name() {
        let struct_type = TypeRepr::Struct {
            name: "Point".to_string(),
            fields: vec![
                FieldRepr {
                    name: "x".to_string(),
                    type_repr: Box::new(TypeRepr::i32()),
                    index: 0,
                },
                FieldRepr {
                    name: "y".to_string(),
                    type_repr: Box::new(TypeRepr::i32()),
                    index: 1,
                },
            ],
            type_params: vec![],
            alignment: 4,
        };
        assert_eq!(struct_type.type_name(), "Point");

        // With type params
        let generic = TypeRepr::Struct {
            name: "Vec".to_string(),
            fields: vec![],
            type_params: vec![TypeRepr::i32()],
            alignment: 8,
        };
        assert_eq!(generic.type_name(), "Vec<i32>");
    }

    #[test]
    fn test_type_equality() {
        let a = TypeRepr::i32();
        let b = TypeRepr::i32();
        assert_eq!(a, b);

        let c = TypeRepr::i64();
        assert_ne!(a, c);

        let t1 = TypeRepr::Tuple(vec![TypeRepr::i32(), TypeRepr::i64()]);
        let t2 = TypeRepr::Tuple(vec![TypeRepr::i32(), TypeRepr::i64()]);
        assert_eq!(t1, t2);

        let t3 = TypeRepr::Tuple(vec![TypeRepr::i64(), TypeRepr::i32()]);
        assert_ne!(t1, t3);
    }

    #[test]
    fn test_struct_field_access() {
        let point = TypeRepr::Struct {
            name: "Point".to_string(),
            fields: vec![
                FieldRepr {
                    name: "x".to_string(),
                    type_repr: Box::new(TypeRepr::i32()),
                    index: 0,
                },
                FieldRepr {
                    name: "y".to_string(),
                    type_repr: Box::new(TypeRepr::i32()),
                    index: 1,
                },
            ],
            type_params: vec![],
            alignment: 4,
        };

        assert!(point.has_field("x"));
        assert!(point.has_field("y"));
        assert!(!point.has_field("z"));

        assert_eq!(point.field_count(), 2);

        let x_field = point.get_field("x").unwrap();
        assert_eq!(x_field.name, "x");
        assert_eq!(x_field.type_repr.type_name(), "i32");
        assert_eq!(x_field.index, 0);
    }

    #[test]
    fn test_primitive_size() {
        assert_eq!(TypeRepr::i32().calculate_size(&crate::SemanticContext::new()), 4);
        assert_eq!(TypeRepr::i64().calculate_size(&crate::SemanticContext::new()), 8);
        assert_eq!(TypeRepr::bool().calculate_size(&crate::SemanticContext::new()), 1);
        assert_eq!(TypeRepr::void().calculate_size(&crate::SemanticContext::new()), 0);
    }

    #[test]
    fn test_int_repr() {
        assert!(IntRepr::I32.is_signed());
        assert!(!IntRepr::U32.is_signed());
        assert_eq!(IntRepr::I32.bit_width(), 32);
        assert_eq!(IntRepr::U64.bit_width(), 64);
    }

    #[test]
    fn test_type_kind() {
        let i32_type = TypeRepr::i32();
        let info = i32_type.type_info(&crate::SemanticContext::new());
        assert!(matches!(info.kind, TypeKind::Primitive));

        let arr = TypeRepr::Array {
            element: Box::new(TypeRepr::i32()),
            length: 10,
        };
        let info = arr.type_info(&crate::SemanticContext::new());
        assert!(matches!(info.kind, TypeKind::Array));
    }
}
