// TODO
//#![no_std]

extern crate alloc;

use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::ops::{Add, Div, Mul, Sub};
use core::str::Chars;
use nalgebra::{Matrix3, Matrix4, Vector3, Vector4};

pub const BYTE_BIT_WIDTH: u32 = 8;
type ByteCount = u8;
type SizeCount = u32;

pub trait ToPrimitiveType {
    fn to_type(&self) -> PrimitiveType;
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PrimitiveType {
    Unit, // void
    Boolean,
    Integer {
        signed: bool,
        byte: ByteCount,
    },
    Float {
        byte: ByteCount,
    },
    Struct {
        elements: Vec<PrimitiveType>,
    },
    /*
    Union {
        max_size: SizeCount,
        elements: Vec<PrimitiveType>,
    },
    * */
    Enumeration {
        variants: Vec<PrimitiveType>,
    },
    Array {
        type_kind: Box<PrimitiveType>,
        size: SizeCount,
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
        dimension: SizeCount,
    },
    Matrix {
        type_kind: Box<PrimitiveType>,
        rows: SizeCount,
        columns: SizeCount,
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

    pub fn is_scalar_type(&self) -> bool {
        matches!(self, Self::Integer { .. } | Self::Float { .. })
    }

    pub fn is_unit(&self) -> bool {
        matches!(self, Self::Unit)
    }

    pub fn is_integer(&self) -> bool {
        matches!(self, Self::Integer { .. })
    }

    pub fn is_float(&self) -> bool {
        matches!(self, Self::Float { .. })
    }

    pub fn is_boolean(&self) -> bool {
        matches!(self, Self::Boolean)
    }

    pub fn is_function(&self) -> bool {
        matches!(self, Self::Function { .. })
    }

    pub fn is_struct(&self) -> bool {
        matches!(self, Self::Struct { .. })
    }

    pub fn is_array(&self) -> bool {
        matches!(self, Self::Array { .. })
    }

    pub fn is_pointer(&self) -> bool {
        matches!(self, Self::Pointer { .. })
    }

    pub fn is_valid(&self) -> bool {
        match self {
            Self::Unit | Self::Boolean => true,
            Self::Integer { byte, .. } | Self::Float { byte } => *byte > 0,
            Self::Struct { elements } => {
                for element in elements {
                    if !element.is_valid() {
                        return false;
                    }
                }
                true
            }
            Self::Enumeration { variants } => {
                for variant in variants {
                    if !variant.is_valid() {
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
            } => type_kind.is_scalar_type() && type_kind.is_valid() && *dimension > 0,
            Self::Matrix {
                type_kind,
                rows,
                columns,
            } => type_kind.is_valid() && type_kind.is_scalar_type() && *rows > 0 && *columns > 0,
        }
    }

    pub fn from(name: &str) -> Option<Self> {
        if let Some(postfix) = name.strip_prefix("i") {
            let mut chars = postfix.chars();
            let bit_width = Self::parse_number(&mut chars)?;
            let byte = Self::get_byte(bit_width)?;
            return Some(Self::Integer { signed: true, byte });
        }

        if let Some(postfix) = name.strip_prefix("u") {
            let mut chars = postfix.chars();
            let bit_width = Self::parse_number(&mut chars)?;
            let byte = Self::get_byte(bit_width)?;
            return Some(Self::Integer {
                signed: false,
                byte,
            });
        }

        if let Some(postfix) = name.strip_prefix("f") {
            let mut chars = postfix.chars();
            let bit_width = Self::parse_number(&mut chars)?;
            let byte = Self::get_byte(bit_width)?;
            return Some(Self::Float { byte });
        }

        // ベクトル
        if let Some(postfix) = name.strip_prefix("vec") {
            return Self::parse_vec(postfix);
        }

        // 行列
        if let Some(postfix) = name.strip_prefix("mat") {
            return Self::parse_matrix(postfix);
        }
        None
    }

    // vec2i16, vec3, vec4f32
    fn parse_vec(postfix: &str) -> Option<Self> {
        let mut chars = postfix.chars();
        let dimension = Self::parse_number(&mut chars)?;

        // 型指定がなければf32として扱う
        let spec_ty = chars.as_str();
        if spec_ty.is_empty() {
            return Some(Self::Vector {
                dimension,
                type_kind: Box::new(PrimitiveType::default_float()),
            });
        }

        // 内部の型
        let ty = Self::from(spec_ty)?;
        if !ty.is_scalar_type() {
            return None;
        }

        return Some(Self::Vector {
            dimension,
            type_kind: Box::new(ty),
        });
    }

    // 最初は列を表し次に行を表す
    // mat3x3, mat4x3f32, mat4x4i32
    fn parse_matrix(postfix: &str) -> Option<Self> {
        let mut chars = postfix.chars();

        let columns = Self::parse_number(&mut chars)?;
        if !matches!(chars.next(), Some('x')) {
            return None;
        }
        let rows = Self::parse_number(&mut chars)?;

        // 型
        // 型なしはvec同様f32として扱う
        let spec_ty = chars.as_str();
        if spec_ty.is_empty() {
            return Some(Self::Matrix {
                columns,
                rows,
                type_kind: Box::new(PrimitiveType::default_float()),
            });
        }
        let ty = Self::from(spec_ty)?;
        if !ty.is_scalar_type() {
            return None;
        }

        return Some(Self::Matrix {
            columns,
            rows,
            type_kind: Box::new(ty),
        });
    }

    fn parse_number(iter: &mut Chars) -> Option<u32> {
        let s = iter.as_str();
        let end_index = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());

        if end_index == 0 {
            return None;
        }

        let num_str = &s[..end_index];
        let number = num_str.parse::<SizeCount>().ok()?;

        for _ in 0..num_str.chars().count() {
            iter.next();
        }

        Some(number)
    }

    fn get_byte(bit_width: SizeCount) -> Option<ByteCount> {
        if bit_width % BYTE_BIT_WIDTH != 0 {
            return None;
        }
        let byte = bit_width / BYTE_BIT_WIDTH;
        if (ByteCount::MAX as SizeCount) < byte {
            return None;
        }
        Some(byte as ByteCount)
    }
}

impl ToString for PrimitiveType {
    fn to_string(&self) -> String {
        match self {
            Self::Unit => "void".to_string(),
            Self::Boolean => "bool".to_string(),
            Self::Integer { signed, byte } => {
                let bit = (*byte as SizeCount) * BYTE_BIT_WIDTH;
                if *signed {
                    format!("int_{bit}")
                } else {
                    format!("uint_{bit}")
                }
            }
            Self::Float { byte } => format!("float_{}", (*byte as SizeCount) * BYTE_BIT_WIDTH),
            Self::Struct { elements } => elements.iter().map(|elem| elem.to_string()).collect(),
            Self::Enumeration { variants } => {
                variants.iter().map(|variant| variant.to_string()).collect()
            }
            Self::Array { type_kind, size } => format!("{}[{size}]", type_kind.to_string()),
            Self::Pointer { point } => format!("{}_ptr", point.to_string()),
            Self::Function {
                return_type,
                arguments,
            } => format!(
                "fn({})->{}",
                arguments
                    .iter()
                    .map(|arg| arg.to_string())
                    .collect::<Vec<String>>()
                    .join("_"),
                return_type.to_string()
            ),
            Self::Vector {
                type_kind,
                dimension,
            } => format!("vec<{}>{dimension}", type_kind.to_string()),
            Self::Matrix {
                type_kind,
                rows,
                columns,
            } => format!("matrix<{}>{rows}x{columns}", type_kind.to_string()),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Value {
    Integer(Integer),
    Float(Float),
    Bool(bool),
    Pointer,
    String(String),
    Vector(Vector),
    Matrix(Matrix),
}

impl ToPrimitiveType for Value {
    fn to_type(&self) -> PrimitiveType {
        match self {
            Self::Integer(integer) => integer.to_type(),
            Self::Float(float) => float.to_type(),
            Self::Bool(_) => PrimitiveType::Boolean,
            _ => unimplemented!(),
        }
    }
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

impl ToPrimitiveType for Integer {
    fn to_type(&self) -> PrimitiveType {
        match self {
            Self::Int8(_) => PrimitiveType::Integer {
                signed: true,
                byte: 1,
            },
            Self::Int16(_) => PrimitiveType::Integer {
                signed: true,
                byte: 2,
            },
            Self::Int32(_) => PrimitiveType::Integer {
                signed: true,
                byte: 4,
            },
            Self::Int64(_) => PrimitiveType::Integer {
                signed: true,
                byte: 8,
            },
            Self::Uint8(_) => PrimitiveType::Integer {
                signed: false,
                byte: 1,
            },
            Self::Uint16(_) => PrimitiveType::Integer {
                signed: false,
                byte: 2,
            },
            Self::Uint32(_) => PrimitiveType::Integer {
                signed: false,
                byte: 4,
            },
            Self::Uint64(_) => PrimitiveType::Integer {
                signed: false,
                byte: 8,
            },
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

impl ToPrimitiveType for Float {
    fn to_type(&self) -> PrimitiveType {
        match self {
            Self::Float32(_) => PrimitiveType::Float { byte: 4 },
            Self::Float64(_) => PrimitiveType::Float { byte: 8 },
        }
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LogicalOperator {
    Not, // !
    And, // &&
    Or,  // ||
}
