use musubu_ast::{AssignOperator, Literal, LogicalOperator, TypeKind};
use musubu_primitive::*;
use musubu_span::*;
use std::hash::Hash;

use crate::{errors::ParseError, lexer::musubu_keywords::MusubuKeyword};

#[derive(Debug, Hash, Clone, Eq, PartialEq)]
pub struct MusubuToken {
    pub token_kind: MusubuTokenKind,
    pub position: usize,
}

#[derive(Debug, Hash, Clone, Eq, PartialEq)]
pub enum MusubuTokenKind {
    Identifier(String),
    Keyword(MusubuKeyword),
    Literal(MusubuLiteral),
    Operator(MusubuOperator),
}

#[derive(Debug, Hash, Clone, Eq, PartialEq)]
pub enum MusubuLiteral {
    Integer {
        value: String,
        suffix: Option<String>,
    },
    Float(FloatLiteral),
    String(String),
}

impl MusubuLiteral {
    pub fn to_literal(self) -> Result<Literal, ParseError> {
        let make_type = |suffix: Option<String>, default_type: TypeKind| -> TypeKind {
            suffix
                .map(|s| TypeKind::make_single_type(s, Span::default()))
                .unwrap_or(default_type)
        };

        let literal = match self {
            Self::Integer { value, suffix } => Literal::Integer {
                value,
                value_type: make_type(
                    suffix,
                    TypeKind::Primitive(PrimitiveType::Integer {
                        signed: true,
                        byte: 4,
                    }),
                ),
            },
            MusubuLiteral::Float(float_value) => match float_value {
                FloatLiteral::Float { value, suffix } => Literal::Float {
                    value,
                    value_type: make_type(
                        suffix,
                        TypeKind::Primitive(PrimitiveType::Float { byte: 4 }),
                    ),
                },
                FloatLiteral::Exponent {
                    is_plus_exponent,
                    significand,
                    exponent,
                    suffix,
                } => Literal::Float {
                    value: || -> Result<String, ParseError> {
                        // TODO: 二重パースの解消
                        let mantissa: f64 = significand.parse()?;
                        let exponent: i32 = exponent.parse()?;
                        let sign = if is_plus_exponent { 1 } else { -1 };
                        let result = mantissa * 10.0_f64.powi(exponent * sign);
                        Ok(result.to_string())
                    }()?,
                    value_type: make_type(
                        suffix,
                        TypeKind::Primitive(PrimitiveType::Float { byte: 4 }),
                    ),
                },
            },
            MusubuLiteral::String(value) => Literal::String {
                value,
                value_type: TypeKind::Primitive(PrimitiveType::Pointer {
                    point: Box::new(PrimitiveType::Integer {
                        signed: false,
                        byte: 1,
                    }),
                }),
            },
        };

        Ok(literal)
    }
}

#[derive(Debug, Hash, Clone, Eq, PartialEq)]
pub enum FloatLiteral {
    Float {
        value: String,
        suffix: Option<String>,
    },
    Exponent {
        is_plus_exponent: bool,
        significand: String, // 仮数部
        exponent: String,    // 指数部
        suffix: Option<String>,
    },
}

#[derive(Debug, Hash, Clone, Eq, PartialEq)]
pub enum MusubuOperator {
    Binary(BinaryOperator),
    Assign(AssignOperator),
    Comparison(ComparisonOperator),
    Logical(LogicalOperator),

    Question,         // ?
    Dot,              // .
    DotDot,           // ..
    DotDotDot,        // ...
    LeftArrow,        // <-
    RightArrow,       // ->
    Path,             // ::
    LeftParenthesis,  // (
    RightParenthesis, // )
    LeftBrackets,     // [
    RightBrackets,    // ]
    LeftBrace,        // {
    RightBrace,       // }
    Colon,            // :
    Semicolon,        // ;
    Comma,            // ,
    At,               // @
    Underscore,       // _
}

pub type BindingPower = u16;

impl MusubuOperator {
    pub fn enable_operator(&self) -> bool {
        !matches!(self, Self::Comma | Self::Semicolon | Self::Colon)
    }

    pub fn get_prefix_binding_power(&self) -> Option<BindingPower> {
        let bp = match self {
            Self::Binary(BinaryOperator::Subtract) | Self::Logical(LogicalOperator::Not) => 170,
            _ => return None,
        };
        Some(bp)
    }

    pub fn get_infix_binding_power(&self) -> Option<(u16, u16)> {
        use BinaryOperator::*;

        let bp = match self {
            Self::Dot => (200, 201),

            Self::LeftParenthesis => (190, 191),

            // 二項演算子
            Self::Binary(op) => match op {
                Multiply | Divide | Modulo => (160, 161),
                Addition | Subtract => (159, 160),
                LeftShift | RightShift => (158, 159),
                And => (157, 158),
                Xor => (156, 157),
                Or => (155, 156),
            },

            // 比較
            Self::Comparison(_) => (140, 141),

            // 論理
            Self::Logical(op) => match op {
                LogicalOperator::And => (130, 131),
                LogicalOperator::Or => (129, 130),
                _ => return None,
            },

            // 代入演算子(右結合なので L < R)
            Self::Assign(_) => (100, 99),

            _ => return None,
        };

        Some(bp)
    }

    pub fn get_postfix_binding_power(&self) -> Option<u16> {
        let bp = match self {
            Self::LeftParenthesis | Self::LeftBrackets => 190,
            Self::Question => 189,
            _ => return None,
        };

        Some(bp)
    }

    pub fn counterpart_of(&self) -> Option<Self> {
        let pair = match self {
            Self::LeftParenthesis => Self::RightParenthesis,
            Self::LeftBrackets => Self::RightBrackets,
            Self::LeftBrace => Self::RightBrace,
            _ => return None,
        };

        Some(pair)
    }
}
