use nalgebra::{Matrix3, Matrix4, Vector3, Vector4};
use std::ops::{Add, Div, Mul, Sub};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PrimitiveType {
    Unit, // void
    Integer {
        signed: bool,
        byte: u8,
    },
    Float {
        byte: u8,
    },
    Struct {
        elements: Vec<PrimitiveType>,
    },
    Array {
        type_kind: Box<PrimitiveType>,
        size: u32,
    },
    Pointer {
        point: Box<PrimitiveType>,
    },
    Function {
        return_type: Box<PrimitiveType>,
        arguments: Vec<PrimitiveType>,
    },
    Vector {
        type_kind: Box<PrimitiveType>,
        dimension: u32,
    },
    Matrix {
        type_kind: Box<PrimitiveType>,
        rows: u32,
        columns: u32,
    },
}

impl PrimitiveType {
    pub fn default_integer() -> Self {
        Self::Integer {
            signed: true,
            byte: 4,
        }
    }

    pub fn default_float() -> Self {
        Self::Float { byte: 4 }
    }

    pub fn is_unit(&self) -> bool {
        matches!(self, Self::Unit)
    }

    pub fn is_valid(&self) -> bool {
        match self {
            Self::Unit => false,
            Self::Integer { byte, .. } => *byte > 0,
            Self::Float { byte } => *byte > 0,
            Self::Struct { elements } => {
                for element in elements {
                    if !element.is_valid() {
                        return false;
                    }
                }
                true
            }
            Self::Array { type_kind, size } => {
                if !type_kind.is_valid() {
                    return false;
                }
                *size > 0
            }
            Self::Pointer { point } => point.is_valid(),
            Self::Function {
                return_type,
                arguments,
            } => {
                if !return_type.is_valid() {
                    return false;
                }
                for arg in arguments {
                    if !arg.is_valid() {
                        return false;
                    }
                }
                true
            }
            Self::Vector {
                type_kind,
                dimension,
            } => {
                if !type_kind.is_valid() {
                    return false;
                }
                if !matches!(**type_kind, Self::Integer { .. } | Self::Float { .. }) {
                    return false;
                }
                *dimension > 0
            }
            Self::Matrix {
                type_kind,
                rows,
                columns,
            } => {
                if !type_kind.is_valid() {
                    return false;
                }
                if !matches!(**type_kind, Self::Integer { .. } | Self::Float { .. }) {
                    return false;
                }
                *rows > 0 && *columns > 0
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum Value {
    Unit,
    Integer(Integer),
    Float(Float),
    Bool(bool),
    String(String),
    Vector(Vector),
    Matrix(Matrix),
}

// TODO f32以外の型を使用できるように
#[derive(Debug, Clone)]
pub enum Vector {
    Vector3(Vector3<f32>),
    Vector4(Vector4<f32>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Matrix {
    Matrix3(Matrix3<f32>),
    Matrix4(Matrix4<f32>),
}

impl Mul for Matrix {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Self::Matrix3(a), Self::Matrix3(b)) => Self::Matrix3(a * b),
            (Self::Matrix4(a), Self::Matrix4(b)) => Self::Matrix4(a * b),
            _ => panic!(),
        }
    }
}

impl<'a, 'b> Mul<&'b Matrix> for &'a Matrix {
    type Output = Matrix;

    fn mul(self, rhs: &'b Matrix) -> Self::Output {
        match (self, rhs) {
            (Matrix::Matrix3(a), Matrix::Matrix3(b)) => Matrix::Matrix3(*a * *b),
            (Matrix::Matrix4(a), Matrix::Matrix4(b)) => Matrix::Matrix4(*a * *b),
            _ => panic!(),
        }
    }
}

const TYPE_MISMATCH: &str = "Type mismatch in Integer operation";
const DIV_BY_ZERO: &str = "Division by zero";

#[derive(Debug, Clone, PartialEq)]
pub enum Integer {
    Int8(i8),
    Int16(i16),
    Int32(i32),
    Int64(i64),
    Uint8(u8),
    Uint16(u16),
    Uint32(u32),
    Uint64(u64),
}

impl Integer {
    pub fn new(value: &str, type_kind: &PrimitiveType) -> Option<Self> {
        let PrimitiveType::Integer { signed, byte } = type_kind else {
            return None;
        };
        let value = match (*signed, *byte) {
            (true, 1) => Self::Int8(value.parse().ok()?),
            (true, 2) => Self::Int16(value.parse().ok()?),
            (true, 4) => Self::Int32(value.parse().ok()?),
            (true, 8) => Self::Int64(value.parse().ok()?),
            (false, 1) => Self::Uint8(value.parse().ok()?),
            (false, 2) => Self::Uint16(value.parse().ok()?),
            (false, 4) => Self::Uint32(value.parse().ok()?),
            (false, 8) => Self::Uint64(value.parse().ok()?),
            _ => return None,
        };
        Some(value)
    }

    pub fn is_zero(&self) -> bool {
        match self {
            Self::Int8(v) => *v == 0,
            Self::Int16(v) => *v == 0,
            Self::Int32(v) => *v == 0,
            Self::Int64(v) => *v == 0,
            Self::Uint8(v) => *v == 0,
            Self::Uint16(v) => *v == 0,
            Self::Uint32(v) => *v == 0,
            Self::Uint64(v) => *v == 0,
        }
    }
}

impl Add for Integer {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Self::Int8(a), Self::Int8(b)) => Self::Int8(a + b),
            (Self::Int16(a), Self::Int16(b)) => Self::Int16(a + b),
            (Self::Int32(a), Self::Int32(b)) => Self::Int32(a + b),
            (Self::Int64(a), Self::Int64(b)) => Self::Int64(a + b),
            (Self::Uint8(a), Self::Uint8(b)) => Self::Uint8(a + b),
            (Self::Uint16(a), Self::Uint16(b)) => Self::Uint16(a + b),
            (Self::Uint32(a), Self::Uint32(b)) => Self::Uint32(a + b),
            (Self::Uint64(a), Self::Uint64(b)) => Self::Uint64(a + b),
            _ => panic!("{}", TYPE_MISMATCH),
        }
    }
}
impl Sub for Integer {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Self::Int8(a), Self::Int8(b)) => Self::Int8(a - b),
            (Self::Int16(a), Self::Int16(b)) => Self::Int16(a - b),
            (Self::Int32(a), Self::Int32(b)) => Self::Int32(a - b),
            (Self::Int64(a), Self::Int64(b)) => Self::Int64(a - b),
            (Self::Uint8(a), Self::Uint8(b)) => Self::Uint8(a - b),
            (Self::Uint16(a), Self::Uint16(b)) => Self::Uint16(a - b),
            (Self::Uint32(a), Self::Uint32(b)) => Self::Uint32(a - b),
            (Self::Uint64(a), Self::Uint64(b)) => Self::Uint64(a - b),
            _ => panic!("{}", TYPE_MISMATCH),
        }
    }
}

impl Mul for Integer {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Self::Int8(a), Self::Int8(b)) => Self::Int8(a * b),
            (Self::Int16(a), Self::Int16(b)) => Self::Int16(a * b),
            (Self::Int32(a), Self::Int32(b)) => Self::Int32(a * b),
            (Self::Int64(a), Self::Int64(b)) => Self::Int64(a * b),
            (Self::Uint8(a), Self::Uint8(b)) => Self::Uint8(a * b),
            (Self::Uint16(a), Self::Uint16(b)) => Self::Uint16(a * b),
            (Self::Uint32(a), Self::Uint32(b)) => Self::Uint32(a * b),
            (Self::Uint64(a), Self::Uint64(b)) => Self::Uint64(a * b),
            _ => panic!("{}", TYPE_MISMATCH),
        }
    }
}

impl Div for Integer {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        if rhs.clone().is_zero() {
            panic!("{}", DIV_BY_ZERO);
        }

        match (self, rhs) {
            (Self::Int8(a), Self::Int8(b)) => Self::Int8(a / b),
            (Self::Int16(a), Self::Int16(b)) => Self::Int16(a / b),
            (Self::Int32(a), Self::Int32(b)) => Self::Int32(a / b),
            (Self::Int64(a), Self::Int64(b)) => Self::Int64(a / b),
            (Self::Uint8(a), Self::Uint8(b)) => Self::Uint8(a / b),
            (Self::Uint16(a), Self::Uint16(b)) => Self::Uint16(a / b),
            (Self::Uint32(a), Self::Uint32(b)) => Self::Uint32(a / b),
            (Self::Uint64(a), Self::Uint64(b)) => Self::Uint64(a / b),
            _ => panic!("{}", TYPE_MISMATCH),
        }
    }
}

impl<'a, 'b> Add<&'b Integer> for &'a Integer {
    type Output = Integer;

    fn add(self, rhs: &'b Integer) -> Self::Output {
        match (self, rhs) {
            (Integer::Int8(a), Integer::Int8(b)) => Integer::Int8(*a + *b),
            (Integer::Int16(a), Integer::Int16(b)) => Integer::Int16(*a + *b),
            (Integer::Int32(a), Integer::Int32(b)) => Integer::Int32(*a + *b),
            (Integer::Int64(a), Integer::Int64(b)) => Integer::Int64(*a + *b),
            (Integer::Uint8(a), Integer::Uint8(b)) => Integer::Uint8(*a + *b),
            (Integer::Uint16(a), Integer::Uint16(b)) => Integer::Uint16(*a + *b),
            (Integer::Uint32(a), Integer::Uint32(b)) => Integer::Uint32(*a + *b),
            (Integer::Uint64(a), Integer::Uint64(b)) => Integer::Uint64(*a + *b),
            _ => panic!("{}", TYPE_MISMATCH),
        }
    }
}

impl<'a, 'b> Sub<&'b Integer> for &'a Integer {
    type Output = Integer;

    fn sub(self, rhs: &'b Integer) -> Self::Output {
        match (self, rhs) {
            (Integer::Int8(a), Integer::Int8(b)) => Integer::Int8(*a - *b),
            (Integer::Int16(a), Integer::Int16(b)) => Integer::Int16(*a - *b),
            (Integer::Int32(a), Integer::Int32(b)) => Integer::Int32(*a - *b),
            (Integer::Int64(a), Integer::Int64(b)) => Integer::Int64(*a - *b),
            (Integer::Uint8(a), Integer::Uint8(b)) => Integer::Uint8(*a - *b),
            (Integer::Uint16(a), Integer::Uint16(b)) => Integer::Uint16(*a - *b),
            (Integer::Uint32(a), Integer::Uint32(b)) => Integer::Uint32(*a - *b),
            (Integer::Uint64(a), Integer::Uint64(b)) => Integer::Uint64(*a - *b),
            _ => panic!("{}", TYPE_MISMATCH),
        }
    }
}

impl<'a, 'b> Mul<&'b Integer> for &'a Integer {
    type Output = Integer;

    fn mul(self, rhs: &'b Integer) -> Self::Output {
        match (self, rhs) {
            (Integer::Int8(a), Integer::Int8(b)) => Integer::Int8(*a * *b),
            (Integer::Int16(a), Integer::Int16(b)) => Integer::Int16(*a * *b),
            (Integer::Int32(a), Integer::Int32(b)) => Integer::Int32(*a * *b),
            (Integer::Int64(a), Integer::Int64(b)) => Integer::Int64(*a * *b),
            (Integer::Uint8(a), Integer::Uint8(b)) => Integer::Uint8(*a * *b),
            (Integer::Uint16(a), Integer::Uint16(b)) => Integer::Uint16(*a * *b),
            (Integer::Uint32(a), Integer::Uint32(b)) => Integer::Uint32(*a * *b),
            (Integer::Uint64(a), Integer::Uint64(b)) => Integer::Uint64(*a * *b),
            _ => panic!("{}", TYPE_MISMATCH),
        }
    }
}

impl<'a, 'b> Div<&'b Integer> for &'a Integer {
    type Output = Integer;

    fn div(self, rhs: &'b Integer) -> Self::Output {
        if rhs.is_zero() {
            panic!("{}", DIV_BY_ZERO);
        }

        match (self, rhs) {
            (Integer::Int8(a), Integer::Int8(b)) => Integer::Int8(*a / *b),
            (Integer::Int16(a), Integer::Int16(b)) => Integer::Int16(*a / *b),
            (Integer::Int32(a), Integer::Int32(b)) => Integer::Int32(*a / *b),
            (Integer::Int64(a), Integer::Int64(b)) => Integer::Int64(*a / *b),
            (Integer::Uint8(a), Integer::Uint8(b)) => Integer::Uint8(*a / *b),
            (Integer::Uint16(a), Integer::Uint16(b)) => Integer::Uint16(*a / *b),
            (Integer::Uint32(a), Integer::Uint32(b)) => Integer::Uint32(*a / *b),
            (Integer::Uint64(a), Integer::Uint64(b)) => Integer::Uint64(*a / *b),
            _ => panic!("{}", TYPE_MISMATCH),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Float {
    Float32(f32),
    Float64(f64),
}

impl Float {
    pub fn new(value: &str, type_kind: &PrimitiveType) -> Option<Self> {
        let PrimitiveType::Float { byte } = type_kind else {
            return None;
        };
        let value = match *byte {
            4 => Self::Float32(value.parse().ok()?),
            8 => Self::Float64(value.parse().ok()?),
            _ => return None,
        };
        Some(value)
    }
}

const TYPE_MISMATCH_FLOAT: &str = "Type mismatch in Float operation";

impl Add for Float {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        match (self, rhs) {
            (Self::Float32(a), Self::Float32(b)) => Self::Float32(a + b),
            (Self::Float64(a), Self::Float64(b)) => Self::Float64(a + b),
            _ => panic!("{}", TYPE_MISMATCH_FLOAT),
        }
    }
}

impl Sub for Float {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        match (self, rhs) {
            (Self::Float32(a), Self::Float32(b)) => Self::Float32(a - b),
            (Self::Float64(a), Self::Float64(b)) => Self::Float64(a - b),
            _ => panic!("{}", TYPE_MISMATCH_FLOAT),
        }
    }
}

impl Mul for Float {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self {
        match (self, rhs) {
            (Self::Float32(a), Self::Float32(b)) => Self::Float32(a * b),
            (Self::Float64(a), Self::Float64(b)) => Self::Float64(a * b),
            _ => panic!("{}", TYPE_MISMATCH_FLOAT),
        }
    }
}

impl Div for Float {
    type Output = Self;

    fn div(self, rhs: Self) -> Self {
        match (self, rhs) {
            (Self::Float32(a), Self::Float32(b)) => Self::Float32(a / b),
            (Self::Float64(a), Self::Float64(b)) => Self::Float64(a / b),
            _ => panic!("{}", TYPE_MISMATCH_FLOAT),
        }
    }
}

impl<'a, 'b> Add<&'b Float> for &'a Float {
    type Output = Float;

    fn add(self, rhs: &'b Float) -> Self::Output {
        match (self, rhs) {
            (Float::Float32(a), Float::Float32(b)) => Float::Float32(*a + *b),
            (Float::Float64(a), Float::Float64(b)) => Float::Float64(*a + *b),
            _ => panic!("{}", TYPE_MISMATCH_FLOAT),
        }
    }
}

impl<'a, 'b> Sub<&'b Float> for &'a Float {
    type Output = Float;

    fn sub(self, rhs: &'b Float) -> Self::Output {
        match (self, rhs) {
            (Float::Float32(a), Float::Float32(b)) => Float::Float32(*a - *b),
            (Float::Float64(a), Float::Float64(b)) => Float::Float64(*a - *b),
            _ => panic!("{}", TYPE_MISMATCH_FLOAT),
        }
    }
}

impl<'a, 'b> Mul<&'b Float> for &'a Float {
    type Output = Float;

    fn mul(self, rhs: &'b Float) -> Self::Output {
        match (self, rhs) {
            (Float::Float32(a), Float::Float32(b)) => Float::Float32(*a * *b),
            (Float::Float64(a), Float::Float64(b)) => Float::Float64(*a * *b),
            _ => panic!("{}", TYPE_MISMATCH_FLOAT),
        }
    }
}

impl<'a, 'b> Div<&'b Float> for &'a Float {
    type Output = Float;

    fn div(self, rhs: &'b Float) -> Self::Output {
        match (self, rhs) {
            (Float::Float32(a), Float::Float32(b)) => Float::Float32(*a / *b),
            (Float::Float64(a), Float::Float64(b)) => Float::Float64(*a / *b),
            _ => panic!("{}", TYPE_MISMATCH_FLOAT),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum BinaryOperator {
    Addition,
    Subtract,
    Multiply,
    Divide,
    Modulo,
    And,
    Or,
    Xor,
    LeftShift,
    RightShift,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ComparisonOperator {
    Equal,            // ==
    NotEqual,         // !=
    LessThan,         // <
    LessThanEqual,    // <=
    GreaterThan,      // >
    GreaterThanEqual, // >=
}
