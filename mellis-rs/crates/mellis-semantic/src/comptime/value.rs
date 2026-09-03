use mellis_common::ids::SymbolId;
use crate::ty::SemanticTypeId;
use super::type_repr::{TypeRepr, TypeKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IntWidth {
    I8,
    I16,
    I32,
    I64,
    I128,
    U8,
    U16,
    U32,
    U64,
    U128,
    USize,
    ISize,
}

impl IntWidth {
    pub fn is_signed(&self) -> bool {
        matches!(
            self,
            IntWidth::I8
                | IntWidth::I16
                | IntWidth::I32
                | IntWidth::I64
                | IntWidth::I128
                | IntWidth::ISize
        )
    }

    pub fn bit_width(&self) -> u32 {
        match self {
            IntWidth::I8 | IntWidth::U8 => 8,
            IntWidth::I16 | IntWidth::U16 => 16,
            IntWidth::I32 | IntWidth::U32 => 32,
            IntWidth::I64 | IntWidth::U64 | IntWidth::USize | IntWidth::ISize => 64,
            IntWidth::I128 | IntWidth::U128 => 128,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FloatWidth {
    F32,
    F64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ComptimeValue {
    Unit,
    Bool(bool),
    Int {
        val: i128,
        width: IntWidth,
    },
    Float {
        val: f64,
        width: FloatWidth,
    },
    Char(char),
    Str(String),
    Tuple(Vec<ComptimeValue>),
    Array {
        elements: Vec<ComptimeValue>,
        elem_ty: Option<SemanticTypeId>,
    },
    Struct {
        symbol: Option<SymbolId>,
        type_name: String,
        fields: Vec<(String, ComptimeValue)>,
    },
    Enum {
        symbol: Option<SymbolId>,
        type_name: String,
        variant_name: String,
        variant_index: u32,
        payload: Vec<ComptimeValue>,
    },
    Type(TypeRepr),
    TypeInfo {
        name: String,
        kind: TypeKind,
        size: u64,
        alignment: u64,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ComptimeError {
    DivisionByZero,
    IntegerOverflow,
    TypeMismatch(String),
    UnsupportedOperation(String),
    StepLimitExceeded(usize),
    RecursionLimitExceeded(usize),
    MemoryLimitExceeded,
    ForbiddenSideEffect(String),
    SymbolNotFound(String),
    UseOfUninitializedOrMoved(String),
    PointerEscape(String),
    ComptimeAwaitForbidden,
    Custom(String),
}

impl std::fmt::Display for ComptimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ComptimeError::DivisionByZero => write!(f, "attempt to divide by zero in comptime evaluation"),
            ComptimeError::IntegerOverflow => write!(f, "integer overflow during comptime evaluation"),
            ComptimeError::TypeMismatch(msg) => write!(f, "comptime type mismatch: {}", msg),
            ComptimeError::UnsupportedOperation(msg) => write!(f, "unsupported comptime operation: {}", msg),
            ComptimeError::StepLimitExceeded(limit) => write!(f, "comptime execution step limit exceeded (limit: {})", limit),
            ComptimeError::RecursionLimitExceeded(limit) => write!(f, "comptime call recursion limit exceeded (limit: {})", limit),
            ComptimeError::MemoryLimitExceeded => write!(f, "comptime memory allocation limit exceeded"),
            ComptimeError::ForbiddenSideEffect(msg) => write!(f, "forbidden side-effect in comptime: {}", msg),
            ComptimeError::SymbolNotFound(name) => write!(f, "symbol '{}' not found in comptime context", name),
            ComptimeError::UseOfUninitializedOrMoved(msg) => write!(f, "use of uninitialized or moved value in comptime: {}", msg),
            ComptimeError::PointerEscape(msg) => write!(f, "pointer escape error in comptime: {}", msg),
            ComptimeError::ComptimeAwaitForbidden => write!(f, "comptime await is not allowed: compile-time domain has no async runtime"),
            ComptimeError::Custom(msg) => write!(f, "comptime error: {}", msg),
        }
    }
}

impl ComptimeValue {
    pub fn int(val: i128, width: IntWidth) -> Self {
        ComptimeValue::Int { val, width }
    }

    pub fn i32(val: i32) -> Self {
        ComptimeValue::Int {
            val: val as i128,
            width: IntWidth::I32,
        }
    }

    pub fn i64(val: i64) -> Self {
        ComptimeValue::Int {
            val: val as i128,
            width: IntWidth::I64,
        }
    }

    pub fn usize(val: usize) -> Self {
        ComptimeValue::Int {
            val: val as i128,
            width: IntWidth::USize,
        }
    }

    pub fn as_i128(&self) -> Option<i128> {
        match self {
            ComptimeValue::Int { val, .. } => Some(*val),
            _ => None,
        }
    }

    pub fn as_usize(&self) -> Option<usize> {
        match self {
            ComptimeValue::Int { val, .. } if *val >= 0 => Some(*val as usize),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            ComptimeValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn add(&self, other: &Self) -> Result<Self, ComptimeError> {
        match (self, other) {
            (ComptimeValue::Int { val: a, width: w1 }, ComptimeValue::Int { val: b, width: _ }) => {
                let res = a.checked_add(*b).ok_or(ComptimeError::IntegerOverflow)?;
                Ok(ComptimeValue::Int { val: res, width: *w1 })
            }
            (ComptimeValue::Float { val: a, width: w1 }, ComptimeValue::Float { val: b, width: _ }) => {
                Ok(ComptimeValue::Float { val: a + b, width: *w1 })
            }
            (ComptimeValue::Str(a), ComptimeValue::Str(b)) => {
                Ok(ComptimeValue::Str(format!("{}{}", a, b)))
            }
            _ => Err(ComptimeError::TypeMismatch("cannot add non-numeric types".to_string())),
        }
    }

    pub fn sub(&self, other: &Self) -> Result<Self, ComptimeError> {
        match (self, other) {
            (ComptimeValue::Int { val: a, width: w1 }, ComptimeValue::Int { val: b, width: _ }) => {
                let res = a.checked_sub(*b).ok_or(ComptimeError::IntegerOverflow)?;
                Ok(ComptimeValue::Int { val: res, width: *w1 })
            }
            (ComptimeValue::Float { val: a, width: w1 }, ComptimeValue::Float { val: b, width: _ }) => {
                Ok(ComptimeValue::Float { val: a - b, width: *w1 })
            }
            _ => Err(ComptimeError::TypeMismatch("cannot subtract non-numeric types".to_string())),
        }
    }

    pub fn mul(&self, other: &Self) -> Result<Self, ComptimeError> {
        match (self, other) {
            (ComptimeValue::Int { val: a, width: w1 }, ComptimeValue::Int { val: b, width: _ }) => {
                let res = a.checked_mul(*b).ok_or(ComptimeError::IntegerOverflow)?;
                Ok(ComptimeValue::Int { val: res, width: *w1 })
            }
            (ComptimeValue::Float { val: a, width: w1 }, ComptimeValue::Float { val: b, width: _ }) => {
                Ok(ComptimeValue::Float { val: a * b, width: *w1 })
            }
            _ => Err(ComptimeError::TypeMismatch("cannot multiply non-numeric types".to_string())),
        }
    }

    pub fn div(&self, other: &Self) -> Result<Self, ComptimeError> {
        match (self, other) {
            (ComptimeValue::Int { val: a, width: w1 }, ComptimeValue::Int { val: b, width: _ }) => {
                if *b == 0 {
                    return Err(ComptimeError::DivisionByZero);
                }
                let res = a.checked_div(*b).ok_or(ComptimeError::IntegerOverflow)?;
                Ok(ComptimeValue::Int { val: res, width: *w1 })
            }
            (ComptimeValue::Float { val: a, width: w1 }, ComptimeValue::Float { val: b, width: _ }) => {
                if *b == 0.0 {
                    return Err(ComptimeError::DivisionByZero);
                }
                Ok(ComptimeValue::Float { val: a / b, width: *w1 })
            }
            _ => Err(ComptimeError::TypeMismatch("cannot divide non-numeric types".to_string())),
        }
    }

    pub fn rem(&self, other: &Self) -> Result<Self, ComptimeError> {
        match (self, other) {
            (ComptimeValue::Int { val: a, width: w1 }, ComptimeValue::Int { val: b, width: _ }) => {
                if *b == 0 {
                    return Err(ComptimeError::DivisionByZero);
                }
                let res = a.checked_rem(*b).ok_or(ComptimeError::IntegerOverflow)?;
                Ok(ComptimeValue::Int { val: res, width: *w1 })
            }
            _ => Err(ComptimeError::TypeMismatch("cannot modulo non-integer types".to_string())),
        }
    }

    pub fn bit_and(&self, other: &Self) -> Result<Self, ComptimeError> {
        match (self, other) {
            (ComptimeValue::Int { val: a, width: w1 }, ComptimeValue::Int { val: b, width: _ }) => {
                Ok(ComptimeValue::Int { val: a & b, width: *w1 })
            }
            (ComptimeValue::Bool(a), ComptimeValue::Bool(b)) => Ok(ComptimeValue::Bool(*a && *b)),
            _ => Err(ComptimeError::TypeMismatch("invalid types for bitwise and".to_string())),
        }
    }

    pub fn bit_or(&self, other: &Self) -> Result<Self, ComptimeError> {
        match (self, other) {
            (ComptimeValue::Int { val: a, width: w1 }, ComptimeValue::Int { val: b, width: _ }) => {
                Ok(ComptimeValue::Int { val: a | b, width: *w1 })
            }
            (ComptimeValue::Bool(a), ComptimeValue::Bool(b)) => Ok(ComptimeValue::Bool(*a || *b)),
            _ => Err(ComptimeError::TypeMismatch("invalid types for bitwise or".to_string())),
        }
    }

    pub fn bit_xor(&self, other: &Self) -> Result<Self, ComptimeError> {
        match (self, other) {
            (ComptimeValue::Int { val: a, width: w1 }, ComptimeValue::Int { val: b, width: _ }) => {
                Ok(ComptimeValue::Int { val: a ^ b, width: *w1 })
            }
            (ComptimeValue::Bool(a), ComptimeValue::Bool(b)) => Ok(ComptimeValue::Bool(*a ^ *b)),
            _ => Err(ComptimeError::TypeMismatch("invalid types for bitwise xor".to_string())),
        }
    }

    pub fn shl(&self, other: &Self) -> Result<Self, ComptimeError> {
        match (self, other) {
            (ComptimeValue::Int { val: a, width: w1 }, ComptimeValue::Int { val: b, width: _ }) => {
                if *b < 0 || *b >= 128 {
                    return Err(ComptimeError::IntegerOverflow);
                }
                let res = a.checked_shl(*b as u32).ok_or(ComptimeError::IntegerOverflow)?;
                Ok(ComptimeValue::Int { val: res, width: *w1 })
            }
            _ => Err(ComptimeError::TypeMismatch("invalid types for shift left".to_string())),
        }
    }

    pub fn shr(&self, other: &Self) -> Result<Self, ComptimeError> {
        match (self, other) {
            (ComptimeValue::Int { val: a, width: w1 }, ComptimeValue::Int { val: b, width: _ }) => {
                if *b < 0 || *b >= 128 {
                    return Err(ComptimeError::IntegerOverflow);
                }
                let res = a.checked_shr(*b as u32).ok_or(ComptimeError::IntegerOverflow)?;
                Ok(ComptimeValue::Int { val: res, width: *w1 })
            }
            _ => Err(ComptimeError::TypeMismatch("invalid types for shift right".to_string())),
        }
    }

    pub fn neg(&self) -> Result<Self, ComptimeError> {
        match self {
            ComptimeValue::Int { val, width } => {
                let res = val.checked_neg().ok_or(ComptimeError::IntegerOverflow)?;
                Ok(ComptimeValue::Int { val: res, width: *width })
            }
            ComptimeValue::Float { val, width } => Ok(ComptimeValue::Float { val: -val, width: *width }),
            _ => Err(ComptimeError::TypeMismatch("cannot negate non-numeric type".to_string())),
        }
    }

    pub fn not(&self) -> Result<Self, ComptimeError> {
        match self {
            ComptimeValue::Bool(b) => Ok(ComptimeValue::Bool(!b)),
            ComptimeValue::Int { val, width } => Ok(ComptimeValue::Int { val: !val, width: *width }),
            _ => Err(ComptimeError::TypeMismatch("cannot invert non-boolean/integer type".to_string())),
        }
    }

    pub fn cmp_eq(&self, other: &Self) -> bool {
        self == other
    }

    pub fn cmp_lt(&self, other: &Self) -> Result<bool, ComptimeError> {
        match (self, other) {
            (ComptimeValue::Int { val: a, .. }, ComptimeValue::Int { val: b, .. }) => Ok(a < b),
            (ComptimeValue::Float { val: a, .. }, ComptimeValue::Float { val: b, .. }) => Ok(a < b),
            (ComptimeValue::Char(a), ComptimeValue::Char(b)) => Ok(a < b),
            (ComptimeValue::Str(a), ComptimeValue::Str(b)) => Ok(a < b),
            _ => Err(ComptimeError::TypeMismatch("cannot compare types with '<'".to_string())),
        }
    }

    pub fn cmp_le(&self, other: &Self) -> Result<bool, ComptimeError> {
        match (self, other) {
            (ComptimeValue::Int { val: a, .. }, ComptimeValue::Int { val: b, .. }) => Ok(a <= b),
            (ComptimeValue::Float { val: a, .. }, ComptimeValue::Float { val: b, .. }) => Ok(a <= b),
            (ComptimeValue::Char(a), ComptimeValue::Char(b)) => Ok(a <= b),
            (ComptimeValue::Str(a), ComptimeValue::Str(b)) => Ok(a <= b),
            _ => Err(ComptimeError::TypeMismatch("cannot compare types with '<='".to_string())),
        }
    }

    pub fn cmp_gt(&self, other: &Self) -> Result<bool, ComptimeError> {
        match (self, other) {
            (ComptimeValue::Int { val: a, .. }, ComptimeValue::Int { val: b, .. }) => Ok(a > b),
            (ComptimeValue::Float { val: a, .. }, ComptimeValue::Float { val: b, .. }) => Ok(a > b),
            (ComptimeValue::Char(a), ComptimeValue::Char(b)) => Ok(a > b),
            (ComptimeValue::Str(a), ComptimeValue::Str(b)) => Ok(a > b),
            _ => Err(ComptimeError::TypeMismatch("cannot compare types with '>'".to_string())),
        }
    }

    pub fn cmp_ge(&self, other: &Self) -> Result<bool, ComptimeError> {
        match (self, other) {
            (ComptimeValue::Int { val: a, .. }, ComptimeValue::Int { val: b, .. }) => Ok(a >= b),
            (ComptimeValue::Float { val: a, .. }, ComptimeValue::Float { val: b, .. }) => Ok(a >= b),
            (ComptimeValue::Char(a), ComptimeValue::Char(b)) => Ok(a >= b),
            (ComptimeValue::Str(a), ComptimeValue::Str(b)) => Ok(a >= b),
            _ => Err(ComptimeError::TypeMismatch("cannot compare types with '>='".to_string())),
        }
    }

    // =========================================================================
    // Type Operations (for Type as Value support)
    // =========================================================================

    /// Create a ComptimeValue::Type from a TypeRepr.
    pub fn type_value(type_repr: TypeRepr) -> Self {
        ComptimeValue::Type(type_repr)
    }

    /// Create a ComptimeValue::Type from a SemanticTypeId.
    pub fn from_semantic_type(type_id: SemanticTypeId, ctx: &crate::SemanticContext) -> Self {
        ComptimeValue::Type(TypeRepr::from_semantic_type(type_id, ctx))
    }

    /// Get the TypeRepr if this is a Type value.
    pub fn as_type(&self) -> Option<&TypeRepr> {
        match self {
            ComptimeValue::Type(type_repr) => Some(type_repr),
            _ => None,
        }
    }

    /// Get the type name as a string.
    ///
    /// Returns the name for type values, or None for non-type values.
    pub fn type_name(&self) -> Option<String> {
        match self {
            ComptimeValue::Type(type_repr) => Some(type_repr.type_name()),
            _ => None,
        }
    }

    /// Check if this type has a field with the given name.
    pub fn has_field(&self, field_name: &str) -> Option<bool> {
        self.as_type().map(|t| t.has_field(field_name))
    }

    /// Get the type of the given field, if it exists.
    pub fn field_type(&self, field_name: &str) -> Option<ComptimeValue> {
        self.as_type()
            .and_then(|t| t.get_field(field_name))
            .map(|f| ComptimeValue::Type(*f.type_repr.clone()))
    }

    /// Get the number of fields in this type.
    pub fn field_count(&self) -> Option<usize> {
        self.as_type().map(|t| t.field_count())
    }

    /// Get full type info for this type.
    pub fn type_info(&self, ctx: &crate::SemanticContext) -> Option<super::type_repr::TypeInfoStruct> {
        self.as_type().map(|t| t.type_info(ctx))
    }

    /// Check if this value is a type value.
    pub fn is_type(&self) -> bool {
        matches!(self, ComptimeValue::Type(_))
    }

    /// Get sizeof for this type (if it's a type).
    pub fn sizeof(&self, ctx: &crate::SemanticContext) -> Option<usize> {
        self.as_type().map(|t| t.calculate_size(ctx))
    }

    /// Get alignof for this type (if it's a type).
    pub fn alignof(&self, ctx: &crate::SemanticContext) -> Option<usize> {
        self.as_type().map(|t| t.calculate_alignment(ctx))
    }
}
