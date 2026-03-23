use crate::{
    TokenStream,
    errors::ParseError,
    lexer::{
        musubu_keywords::MusubuKeyword,
        token::{BindingPower, FloatLiteral, MusubuLiteral, MusubuOperator, MusubuTokenKind},
    },
};
use musubu_ast::*;
use musubu_primitive::*;
use musubu_span::*;
use std::{collections::HashMap, hash::Hash, rc::Rc};

#[derive(Debug, Clone)]
enum Memo {
    ASTNode(Rc<ASTNode>),
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct MemoKey<'a> {
    rule: &'a str,
    position: usize,
}

#[derive(Debug, Clone)]
enum MemoResult {
    Pending,
    NotMatch,
    Match { memo: Memo, next_position: usize },
}

impl MemoResult {
    fn from_node(node: Rc<ASTNode>, next_position: usize) -> Self {
        Self::Match {
            memo: Memo::ASTNode(node),
            next_position,
        }
    }

    fn get_node(&self) -> Option<Rc<ASTNode>> {
        let MemoResult::Match {
            memo: Memo::ASTNode(node),
            ..
        } = self
        else {
            return None;
        };
        Some(node.clone())
    }
}

#[derive(Debug)]
pub(crate) struct PackratAndPrattParser<'a> {
    memo: HashMap<MemoKey<'a>, MemoResult>,
    tokens: TokenStream,
    max_read_position: usize,
    last_fail_rule: Option<&'a str>,
    bp_stack: Vec<BindingPower>,
}

pub(crate) type ParseResult = Result<MemoResult, ParseError>;

// TODO デカすぎるので分割する(あとマクロで共通処理を纏める)
//
// 仕様上想定していない場合には unreachable! マクロを使用
// それ以外コンパイルエラーとして Err を返す
//
impl<'a> PackratAndPrattParser<'a> {
    pub fn new(tokens: TokenStream) -> Self {
        Self {
            memo: HashMap::new(),
            tokens,
            max_read_position: 0,
            last_fail_rule: None,
            bp_stack: vec![],
        }
    }

    pub fn parse(&mut self) -> Result<Rc<ASTNode>, ParseError> {
        self.memo.clear();

        let item = self
            .parse_item()
            .map(|memo| memo.get_node().ok_or(ParseError::UnexpectedAST));

        let pos = self.max_read_position;
        self.tokens.set_position(pos);
        println!(
            "position: {:?}, rule: {:?}, token: {:?}",
            pos,
            self.last_fail_rule,
            self.tokens.get()
        );

        item?
    }

    // Item ::= VisItem
    fn parse_item(&mut self) -> ParseResult {
        let key = self.make_key("Item");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        let Ok(Some(node)) = self
            .parse_visitem()
            .map(|memo| memo.get_node().as_deref().cloned())
        else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };
        let ASTNode::Item { .. } = &node else {
            unreachable!();
        };

        self.make_memo_from_node(key, Rc::new(node))
    }

    // VisItem ::= Visibility? (
    //           | Function
    //           | Struct
    //           | Enumeration
    //
    // )
    fn parse_visitem(&mut self) -> ParseResult {
        let key = self.make_key("VisItem");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        let vis = self.option(Self::parse_visibility);

        let result = self.or(vec![
            Self::parse_function,
            Self::parse_struct,
            Self::parse_enumeration,
        ]);
        self.make_memo_from_result(key, result)
    }

    fn parse_visibility(&mut self) -> ParseResult {
        let key = self.make_key("Visibility");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }
        // TODO
        self.make_memo_from_result(key, Err(ParseError::NotMatch))
    }

    // Function ::= `fn` IDENTIFIER `(` elseFunctionParameters? `)` FunctionReturnType? ( BlockExpression | `;` )
    fn parse_function(&mut self) -> ParseResult {
        let key = self.make_key("Function");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        // `fn`
        let keyword = self.tokens.get_keyword().ok_or(ParseError::NotMatch)?;
        if !matches!(keyword, MusubuKeyword::Fn) {
            return Err(ParseError::NotMatch);
        }
        self.tokens.next();

        // IDENTIFIER
        let identifier = self
            .tokens
            .get_identifier()
            .ok_or(ParseError::NotMatch)?
            .to_string();
        self.tokens.next();

        // `(`
        if self.tokens.get_operator() != Some(&MusubuOperator::LeftParenthesis) {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        }
        self.tokens.next();

        // FunctionParameters?
        let params = self
            .option(Self::parse_function_parameters)
            .and_then(|memo| memo.get_node())
            .and_then(|node| {
                let ASTNode::FunctionParameters(params) = node.as_ref().clone() else {
                    unreachable!();
                };
                Some(params)
            })
            .unwrap_or_default();

        // `)`
        if self.tokens.get_operator() != Some(&MusubuOperator::RightParenthesis) {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        }
        self.tokens.next();

        // FunctionReturnType?
        let return_type = self
            .option(Self::parse_function_return_type)
            .and_then(|memo| memo.get_node())
            .and_then(|node| {
                let ASTNode::Type(return_type) = node.as_ref().clone() else {
                    unreachable!();
                };
                Some(return_type)
            });

        // ( BlockExpression | `;` )
        let Ok(body) = self.parse_block_expression() else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };
        let Some(ASTNode::Expression(expr)) = body.get_node().as_deref().cloned() else {
            unreachable!();
        };

        let span = self.make_span(&key);

        self.make_memo_from_node(
            key,
            Rc::new(
                Item::Function {
                    name: identifier,
                    params,
                    return_type,
                    body: Some(expr),
                }
                .make_item(
                    Visibility::Public, // 仮
                    span,
                ),
            ),
        )
    }

    // FunctionParameters ::= FunctionParam (`,` FunctionParam)* `,`?
    fn parse_function_parameters(&mut self) -> ParseResult {
        let key = self.make_key("FunctionParameters");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        // FunctionParam
        let Ok(first_param) = self.get_node(Self::parse_function_param) else {
            return self.make_memo_from_result(key, Err(ParseError::UnexpectedAST));
        };

        // (`,` FunctionParam)*
        let params = self.zero_or_more(|parser: &mut Self| -> ParseResult {
            let Some(MusubuOperator::Comma) = parser.tokens.get_operator() else {
                return Err(ParseError::NotMatch);
            };
            parser.tokens.next();
            parser.parse_function_param()
        });
        let mut params = params
            .into_iter()
            .map(|memo| {
                let Some(node) = memo.get_node() else {
                    unreachable!();
                };
                node
            })
            .collect::<Vec<_>>();

        // 結合
        params.insert(0, first_param);

        // 変換
        let params = params
            .into_iter()
            .map(|node| {
                let ASTNode::FunctionParameter(param) = node.as_ref() else {
                    unreachable!()
                };
                param.clone()
            })
            .collect::<Vec<_>>();

        // `,`?
        if let Some(MusubuOperator::Comma) = self.tokens.get_operator() {
            self.tokens.next();
        }

        self.make_memo_from_node(key, Rc::new(ASTNode::FunctionParameters(params)))
    }

    // FunctionParam ::= ( FunctionParamPattern | Type )
    fn parse_function_param(&mut self) -> ParseResult {
        let key = self.make_key("FunctionParam");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        let Ok(result) = self.or(vec![Self::parse_function_param_pattern, Self::parse_type]) else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };
        let Some(node) = result.get_node() else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };

        let span = self.make_span(&key);
        let param = match node.as_ref().clone() {
            ASTNode::Type(param_type) => Spanned {
                node: FunctionParam {
                    pattern: None,
                    param_type,
                },
                span,
            },
            ASTNode::FunctionParameter(param) => param,
            _ => unreachable!(),
        };

        self.make_memo_from_node(key, Rc::new(ASTNode::FunctionParameter(param)))
    }

    // FunctionParamPattern ::= PatternNoTopAlt `:` ( Type | `...` )
    fn parse_function_param_pattern(&mut self) -> ParseResult {
        let key = self.make_key("FunctionParamPattern");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        // PatternNoTopAlt
        let Ok(result) = self.parse_pattern_no_top_alt() else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };
        let Some(node) = result.get_node().as_deref().cloned() else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };
        let ASTNode::Pattern(pattern) = node else {
            unreachable!();
        };
        let pattern = Some(pattern);

        // `:`
        let Some(MusubuOperator::Colon) = self.tokens.get_operator() else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };
        self.tokens.next();

        //TODO
        // (Type | `...`)
        let Ok(result) = self.or(vec![Self::parse_type]) else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };
        let Some(node) = result.get_node().as_deref().cloned() else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };
        let ASTNode::Type(param_type) = node else {
            unreachable!();
        };

        self.make_memo_from(
            key,
            FunctionParam {
                pattern,
                param_type,
            },
        )
    }

    // FunctionReturnType ::= `->` Type
    fn parse_function_return_type(&mut self) -> ParseResult {
        let key = self.make_key("FunctionReturnType");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        // `->`
        let symbol = self.tokens.get_operator().ok_or(ParseError::NotMatch)?;
        if !matches!(symbol, MusubuOperator::RightArrow) {
            return Err(ParseError::NotMatch);
        }
        self.tokens.next();

        // Type
        let return_type = self.parse_type();

        self.make_memo_from_result(key, return_type)
    }

    // Struct ::= StructStruct | TupleStruct
    fn parse_struct(&mut self) -> ParseResult {
        let key = self.make_key("Struct");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        self.or(vec![Self::parse_struct_struct, Self::parse_tuple_struct])
    }

    // StructStruct ::= `struct` IDENTIFIER GenericParams? WhereClause? ( `{` StructFields? `}` | `;` )
    fn parse_struct_struct(&mut self) -> ParseResult {
        let key = self.make_key("StructStruct");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        // `struct`
        let Some(MusubuKeyword::Struct) = self.tokens.get_keyword() else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };
        self.tokens.next();

        // IDENTIFIER
        let name = self
            .tokens
            .get_identifier()
            .ok_or(ParseError::NotMatch)?
            .to_string();
        self.tokens.next();

        // `{`
        if self.tokens.get_operator() != Some(&MusubuOperator::LeftBrace) {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        }
        self.tokens.next();

        // StructFields?
        let fields = self
            .option(Self::parse_struct_fields)
            .and_then(|memo| memo.get_node())
            .and_then(|kind| {
                if let ASTNode::StructFields(fields) = kind.as_ref() {
                    Some(fields.clone())
                } else {
                    None
                }
            })
            .unwrap_or_default();

        // `}`
        if self.tokens.get_operator() != Some(&MusubuOperator::RightBrace) {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        }
        self.tokens.next();

        let span = self.make_span(&key);
        let item = ASTNode::Item {
            visibility: Visibility::Public,
            item: Spanned {
                node: Item::Struct { name, fields },
                span,
            },
        };

        self.make_memo_from_node(key, Rc::new(item))
    }

    // StructFields ::= StructField (`,` StructField)* `,`?
    fn parse_struct_fields(&mut self) -> ParseResult {
        let key = self.make_key("StructFields");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        let Ok(first) = self.get_node(Self::parse_struct_field) else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };

        let mut fields = vec![first];

        let rest = self.zero_or_more(|parser: &mut Self| {
            if let Some(MusubuOperator::Comma) = parser.tokens.get_operator() {
                parser.tokens.next();
            }
            parser.parse_struct_field()
        });

        for memo in rest {
            let Some(kind) = memo.get_node() else {
                continue;
            };
            fields.push(kind);
        }

        if let Some(MusubuOperator::Comma) = self.tokens.get_operator() {
            self.tokens.next();
        }

        let fields = fields
            .into_iter()
            .filter_map(|kind| {
                if let ASTNode::StructField(field) = kind.as_ref() {
                    Some(field.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        self.make_memo_from_node(key, Rc::new(ASTNode::StructFields(fields)))
    }

    // StructField ::= Visibility? IDENTIFIER `:` Type
    fn parse_struct_field(&mut self) -> ParseResult {
        let key = self.make_key("StructField");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        // visibility
        // TODO
        let visibility = Visibility::Public;

        // IDENTIFIER
        let name = self
            .tokens
            .get_identifier()
            .ok_or(ParseError::NotMatch)?
            .to_string();
        self.tokens.next();

        // `:`
        if self.tokens.get_operator() != Some(&MusubuOperator::Colon) {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        }
        self.tokens.next();

        // Type
        let field_type = self
            .get_node(Self::parse_type)
            .ok()
            .and_then(|kind| {
                if let ASTNode::Type(t) = kind.as_ref() {
                    Some(t.clone())
                } else {
                    None
                }
            })
            .ok_or(ParseError::UnexpectedAST)?;

        self.make_memo_from(
            key,
            StructField {
                visibility,
                name,
                field_type,
            },
        )
    }

    // TupleStruct ::= `struct` IDENTIFIER GenericParams? `(` TupleFields? `)` WhereClause? `;`
    fn parse_tuple_struct(&mut self) -> ParseResult {
        let key = self.make_key("TupleStruct");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }
        self.make_memo_from_result(key, Err(ParseError::NotMatch))
    }

    // TupleFields ::= TupleField (`,` TupleField)* `,`?
    fn parse_tuple_fields(&mut self) -> ParseResult {
        let key = self.make_key("TupleFields");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }
        self.make_memo_from_result(key, Err(ParseError::NotMatch))
    }

    // TupleField ::= Visibility? Type
    fn parse_tuple_field(&mut self) -> ParseResult {
        let key = self.make_key("TupleField");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }
        self.make_memo_from_result(key, Err(ParseError::NotMatch))
    }

    // Enumeration ::= `enum` IDENTIFIER GenericParams? WhereClause? `{` EnumItems? `}`
    fn parse_enumeration(&mut self) -> ParseResult {
        let key = self.make_key("Enumeration");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        if !matches!(self.tokens.get_keyword(), Some(MusubuKeyword::Enum)) {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        }
        self.tokens.next();

        let name = self
            .tokens
            .get_identifier()
            .ok_or(ParseError::NotMatch)?
            .to_string();
        self.tokens.next();

        if self.tokens.get_operator() != Some(&MusubuOperator::LeftBrace) {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        }
        self.tokens.next();

        let items = self
            .option(Self::parse_enum_items)
            .and_then(|memo| memo.get_node())
            .and_then(|kind| {
                let ASTNode::EnumItems(items) = kind.as_ref() else {
                    unreachable!();
                };
                Some(items.clone())
            })
            .unwrap_or_default();

        if self.tokens.get_operator() != Some(&MusubuOperator::RightBrace) {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        }
        self.tokens.next();

        let span = self.make_span(&key);
        let item = Item::Enumeration { name, items }.make_item(Visibility::Public, span);
        self.make_memo_from_node(key, Rc::new(item))
    }

    // EnumItems ::= EnumItem ( `,` EnumItem )* `,`?
    fn parse_enum_items(&mut self) -> ParseResult {
        let key = self.make_key("EnumItems");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        let Ok(first) = self.get_node(Self::parse_enum_item) else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };

        let mut items = vec![first];

        let rest = self.zero_or_more(|parser: &mut Self| {
            if let Some(MusubuOperator::Comma) = parser.tokens.get_operator() {
                parser.tokens.next();
            }
            parser.parse_enum_item()
        });

        for memo in rest {
            if let Some(kind) = memo.get_node() {
                items.push(kind);
            }
        }

        let items = items
            .into_iter()
            .filter_map(|kind| {
                if let ASTNode::EnumItem(item) = kind.as_ref() {
                    Some(item.clone())
                } else {
                    None
                }
            })
            .collect();

        self.make_memo_from_node(key, Rc::new(ASTNode::EnumItems(items)))
    }

    // EnumItem ::= Visibility? IDENTIFIER ( EnumItemTuple | EnumItemStruct )? EnumItemDiscriminant?
    fn parse_enum_item(&mut self) -> ParseResult {
        let key = self.make_key("EnumItem");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        let name = self
            .tokens
            .get_identifier()
            .ok_or(ParseError::NotMatch)?
            .to_string();
        self.tokens.next();

        let fields = self
            .option(|parser: &mut Self| -> ParseResult {
                parser.or(vec![
                    Self::parse_enum_item_tuple,
                    Self::parse_enum_item_struct,
                ])
            })
            .and_then(|memo| memo.get_node())
            .and_then(|kind| {
                let ASTNode::StructFields(fields) = kind.as_ref() else {
                    unreachable!()
                };
                Some(fields.clone())
            })
            .unwrap_or_default();

        self.make_memo_from(
            key,
            EnumItem::StructItem {
                visibility: Visibility::Public,
                name,
                fields,
            },
        )
    }

    // EnumItemTuple ::= `(` TupleFields? `)`
    fn parse_enum_item_tuple(&mut self) -> ParseResult {
        let key = self.make_key("EnumItemTuple");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        self.make_memo_from_result(key, Err(ParseError::NotMatch))
    }

    // EnumItemStruct ::= `{` StructFields? `}`
    fn parse_enum_item_struct(&mut self) -> ParseResult {
        let key = self.make_key("EnumItemStruct");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        if self.tokens.get_operator() != Some(&MusubuOperator::LeftBrace) {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        }
        self.tokens.next();

        let fields = self
            .option(Self::parse_struct_fields)
            .and_then(|memo| memo.get_node())
            .and_then(|kind| {
                if let ASTNode::StructFields(fields) = kind.as_ref() {
                    Some(fields.clone())
                } else {
                    None
                }
            })
            .unwrap_or_default();

        if self.tokens.get_operator() != Some(&MusubuOperator::RightBrace) {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        }
        self.tokens.next();

        self.make_memo_from_node(key, Rc::new(ASTNode::StructFields(fields)))
    }

    // EnumItemDiscriminant ::= `=` Expression
    fn parse_enum_item_discriminant(&mut self) -> ParseResult {
        let key = self.make_key("EnumItemDiscriminant");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }
        self.make_memo_from_result(key, Err(ParseError::NotMatch))
    }

    // ConstantItem ::= `const` ( IDENTIFIER | `_` ) `:` Type ( `=` Expression )? `;`

    // Expression ::= ExpressionWithoutBlock | ExpressionWithBlock
    fn parse_expression(&mut self) -> ParseResult {
        let key = self.make_key("Expression");
        let memo = self.get_memo_uncheck(&key);
        match memo {
            Some(MemoResult::Match { .. }) | Some(MemoResult::NotMatch) => return Ok(memo.unwrap()),
            Some(MemoResult::Pending) | None => (),
        };

        let Ok(result) = self.or(vec![
            Self::parse_expression_without_block,
            Self::parse_expression_with_block,
        ]) else {
            return self.make_memo_from_result(key, Err(ParseError::UnexpectedAST));
        };
        let Some(node) = result.get_node().as_deref().cloned() else {
            return self.make_memo_from_result(key, Err(ParseError::UnexpectedAST));
        };
        let ASTNode::Expression(expr) = node else {
            unreachable!("{node:#?}");
        };

        self.make_memo_from_node(key, Rc::new(ASTNode::Expression(expr)))
    }

    // ExpressionWithoutBlock ::= LiteralExpression
    //                          | PathExpression
    //                          | ContinueExpression
    //                          | BreakExpression
    //                          | RangeExpression
    //                          | ReturnExpression
    fn parse_expression_without_block(&mut self) -> ParseResult {
        let key = self.make_key("ExpressionWithoutBlock");
        let memo = self.get_memo_uncheck(&key);
        match memo {
            Some(MemoResult::Match { .. }) | Some(MemoResult::NotMatch) => return Ok(memo.unwrap()),
            Some(MemoResult::Pending) => {
                let result = self.or(vec![
                    Self::parse_literal_expression,
                    Self::parse_path_expression,
                    Self::parse_continue_expression,
                    Self::parse_break_expression,
                    Self::parse_return_expression,
                ]);
                self.make_memo_from_result(key, result)
            }
            None => {
                // Expression + Expression などの演算子用
                let bp = self.bp_stack.pop().unwrap_or(0);
                let result = self.pratt_parse(bp);
                self.make_memo_from_result(key, result)
            }
        }
    }

    // LiteralExpression ::= CHAR_LITERAL
    fn parse_literal_expression(&mut self) -> ParseResult {
        let key = self.make_key("LiteralExpression");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        let literal = if let Some(literal) = self.tokens.get_literal().cloned() {
            self.tokens.next();
            match literal.to_literal() {
                Ok(literal) => literal,
                Err(err) => return self.make_memo_from_result(key, Err(err)),
            }
        } else if let Some(keyword) = self.tokens.get_keyword().cloned() {
            self.tokens.next();
            match keyword {
                MusubuKeyword::True => Literal::Bool(true),
                MusubuKeyword::False => Literal::Bool(false),
                _ => return self.make_memo_from_result(key, Err(ParseError::NotMatch)),
            }
        } else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };

        let span = self.make_span(&key);
        self.make_memo_from(
            key,
            Expression::Literal(Spanned {
                node: literal,
                span,
            }),
        )
    }

    // PathExpression ::= PathInExpression | QualifiedPathInExpression
    fn parse_path_expression(&mut self) -> ParseResult {
        let key = self.make_key("PathExpression");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        // PathInExpression | QualifiedPathInExpression
        let Ok(result) = self.or(vec![
            Self::parse_path_in_expression,
            Self::parse_qualified_path_in_expression,
        ]) else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };
        let Some(node) = result.get_node().as_deref().cloned() else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };
        let ASTNode::Path(path) = node else {
            unreachable!();
        };

        self.make_memo_from(key, Expression::Path(path))
    }

    // PathInExpression ::= `::`? PathExprSegment ( `::` PathExprSegment )*
    fn parse_path_in_expression(&mut self) -> ParseResult {
        let key = self.make_key("PathInExpression");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        // `::`?
        if let Some(MusubuOperator::Path) = self.tokens.get_operator() {
            self.tokens.next();
        }

        // PathExprSegment
        let Ok(first_segment) = self.get_node(Self::parse_path_expr_segment) else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };

        //  ( `::` PathExprSegment )*
        let mut segments = self
            .zero_or_more(|parser: &mut Self| -> ParseResult {
                if let Some(MusubuOperator::Path) = parser.tokens.get_operator() {
                    parser.tokens.next();
                }
                parser.parse_path_expr_segment()
            })
            .into_iter()
            .map(|memo| {
                let Some(node) = memo.get_node() else {
                    unreachable!()
                };
                node
            })
            .collect::<Vec<_>>();

        // 結合
        segments.insert(0, first_segment);

        // 変換
        let segments = segments
            .into_iter()
            .map(|component| {
                let ASTNode::PathSegment(param) = component.as_ref() else {
                    unreachable!();
                };
                param.clone()
            })
            .collect::<Vec<_>>();

        self.make_memo_from(key, Path { segments })
    }

    // PathExprSegment ::= PathIdentSegment ( `::` GenericArgs )?
    fn parse_path_expr_segment(&mut self) -> ParseResult {
        let key = self.make_key("PathExprSegment");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        // PathIdentSegment
        let Ok(result) = self.get_node(Self::parse_path_ident_segment) else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };
        let ASTNode::Segment(segment) = result.as_ref() else {
            unreachable!();
        };

        // ( `::` GenericArgs )?
        let arguments = self
            .option(|parser: &mut Self| -> ParseResult {
                let Some(MusubuOperator::Path) = parser.tokens.get_operator() else {
                    return Err(ParseError::NotMatch);
                };
                parser.tokens.next();
                parser.parse_generic_args()
            })
            .and_then(|memo| memo.get_node())
            .and_then(|kind| {
                let ASTNode::Arguments(args) = kind.as_ref() else {
                    unreachable!();
                };
                Some(args.clone())
            })
            .unwrap_or_default();

        self.make_memo_from(
            key,
            PathSegment {
                ident: segment.node.clone(),
                arguments,
            },
        )
    }

    // PathIdentSegment ::= IDENTIFIER
    fn parse_path_ident_segment(&mut self) -> ParseResult {
        let key = self.make_key("PathIdentSegment");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        let Some(ident) = self.tokens.get_identifier() else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };
        let ident = ident.to_string();
        self.tokens.next();

        let span = self.make_span(&key);
        self.make_memo_from_node(
            key,
            Rc::new(ASTNode::Segment(Spanned { node: ident, span })),
        )
    }

    // GenericArgs ::= `<` `>` | `<` ( GenericArg `,` )* GenericArg `,`? `>`
    fn parse_generic_args(&mut self) -> ParseResult {
        let key = self.make_key("GenericArgs");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }
        let result = self.or(vec![
            // `<` `>`
            |parser: &mut Self| -> ParseResult {
                let Some(ComparisonOperator::LessThan) = parser.tokens.get_comparison_operator()
                else {
                    return Err(ParseError::NotMatch);
                };
                parser.tokens.next();
                let Some(ComparisonOperator::GreaterThan) = parser.tokens.get_comparison_operator()
                else {
                    return Err(ParseError::NotMatch);
                };
                parser.tokens.next();
                Ok(MemoResult::from_node(
                    Rc::new(ASTNode::Arguments(vec![])),
                    parser.tokens.get_position(),
                ))
            },
            // `<` ( GenericArg `,` )* GenericArg `,`? `>`
            |parser: &mut Self| -> ParseResult {
                let Some(ComparisonOperator::LessThan) = parser.tokens.get_comparison_operator()
                else {
                    return Err(ParseError::NotMatch);
                };
                parser.tokens.next();

                // ( GenericArg `,` )*
                let args = parser.zero_or_more(|parser: &mut Self| -> ParseResult {
                    let arg = parser.parse_generic_arg()?;
                    let Some(MusubuOperator::Comma) = parser.tokens.get_operator() else {
                        return Err(ParseError::NotMatch);
                    };
                    parser.tokens.next();
                    Ok(arg)
                });

                let Some(ComparisonOperator::GreaterThan) = parser.tokens.get_comparison_operator()
                else {
                    return Err(ParseError::NotMatch);
                };
                parser.tokens.next();

                // TODO
                Ok(MemoResult::from_node(
                    Rc::new(ASTNode::Arguments(vec![])),
                    parser.tokens.get_position(),
                ))
            },
        ]);

        result
    }

    // GenericArg ::= Type | GenericArgsConst | GenericArgsBinding | GenericArgsBounds
    fn parse_generic_arg(&mut self) -> ParseResult {
        let key = self.make_key("GenericArg");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }
        let result = self.or(vec![
            Self::parse_type,
            Self::parse_generic_args_const,
            Self::parse_generic_args_binding,
            Self::parse_generic_args_bounds,
        ]);
        self.make_memo_from_result(key, result)
    }

    // GenericArgsConst ::= BlockExpression | LiteralExpression | `-` LiteralExpression | SimplePathSegment
    fn parse_generic_args_const(&mut self) -> ParseResult {
        let key = self.make_key("GenericArgsConst");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }
        let result = self.or(vec![
            Self::parse_block_expression,
            Self::parse_literal_expression,
            Self::parse_simple_path_segment,
        ]);
        self.make_memo_from_result(key, result)
    }

    // GenericArgsBinding ::= IDENTIFIER GenericArgs? `=` Type
    fn parse_generic_args_binding(&mut self) -> ParseResult {
        let key = self.make_key("GenericArgsBinding");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        // IDENTIFIER
        let Some(ident) = self.tokens.get_identifier() else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };
        self.tokens.next();

        self.option(Self::parse_generic_args);

        let Some(AssignOperator::Assign) = self.tokens.get_assign_operator() else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };
        self.tokens.next();

        let result = self.parse_type();

        self.make_memo_from_result(key, result)
    }

    // GenericArgsBounds ::= IDENTIFIER GenericArgs? `:` TypeParamBounds
    fn parse_generic_args_bounds(&mut self) -> ParseResult {
        let key = self.make_key("GenericArgsBounds");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        let Some(ident) = self.tokens.get_identifier() else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };
        self.tokens.next();

        self.option(Self::parse_generic_args);

        let Some(MusubuOperator::Colon) = self.tokens.get_operator() else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };
        self.tokens.next();

        let result = self.parse_type();

        self.make_memo_from_result(key, result)
    }

    // SimplePath ::= `::`? SimplePathSegment ( `::` SimplePathSegment )*
    fn parse_simple_path(&mut self) -> ParseResult {
        let key = self.make_key("SimplePath");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        // `::`?
        if let Some(MusubuOperator::Path) = self.tokens.get_operator() {
            self.tokens.next();
        }

        // SimplePathSegment
        let Ok(first) = self.get_node(Self::parse_simple_path_segment) else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };

        // (`::` SimplePathSegment)*
        let Some(mut segments) = self
            .zero_or_more(|parser: &mut Self| -> ParseResult {
                if let Some(MusubuOperator::Path) = parser.tokens.get_operator() {
                    parser.tokens.next();
                }
                parser.parse_simple_path_segment()
            })
            .into_iter()
            .map(|memo| memo.get_node())
            .collect::<Option<Vec<_>>>()
        else {
            return self.make_memo_from_result(key, Err(ParseError::UnexpectedAST));
        };

        segments.insert(0, first);

        let segments = segments
            .into_iter()
            .map(|seg| {
                let ASTNode::Segment(segment) = seg.as_ref() else {
                    unreachable!();
                };
                Spanned {
                    node: PathSegment {
                        ident: segment.node.clone(),
                        arguments: vec![],
                    },
                    span: segment.span,
                }
            })
            .collect::<Vec<_>>();

        self.make_memo_from(key, Path { segments })
    }

    // SimplePathSegment ::= IDENTIFIER
    fn parse_simple_path_segment(&mut self) -> ParseResult {
        let key = self.make_key("SimplePathSegment");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        // IDENTIFIER
        let Some(ident) = self.tokens.get_identifier() else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };
        let ident = ident.to_string();
        self.tokens.next();

        let span = self.make_span(&key);
        let node = ASTNode::Segment(Spanned { node: ident, span });
        self.make_memo_from_node(key, Rc::new(node))
    }

    // QualifiedPathInExpression ::= QualifiedPathType (`::` PathExprSegment)+
    fn parse_qualified_path_in_expression(&mut self) -> ParseResult {
        let key = self.make_key("QualifiedPathInExpression");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        // QualifiedPathType
        let Ok(_) = self.parse_type() else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };

        let mut segments = vec![];

        loop {
            if let Some(MusubuOperator::Path) = self.tokens.get_operator() {
                self.tokens.next();
            } else {
                break;
            }

            let Ok(seg) = self.get_node(Self::parse_path_expr_segment) else {
                return self.make_memo_from_result(key, Err(ParseError::UnexpectedAST));
            };

            let ASTNode::PathSegment(segment) = seg.as_ref() else {
                return self.make_memo_from_result(key, Err(ParseError::UnexpectedAST));
            };

            segments.push(segment.clone());
        }

        self.make_memo_from(key, Path { segments })
    }

    // QualifiedPathType ::= `<` Type (`as` TypePath)? `>`
    fn parse_qualified_path_type(&mut self) -> ParseResult {
        let key = self.make_key("OperatorExpression");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        // TODO
        let result = self.parse_type();

        self.make_memo_from_result(key, result)
    }

    // ContinueExpression ::= `continue` LIFETIME_OR_LABEL?
    fn parse_continue_expression(&mut self) -> ParseResult {
        let key = self.make_key("ContinueExpression");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        // `continue`
        if !matches!(self.tokens.get_keyword(), Some(MusubuKeyword::Continue)) {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        }
        self.tokens.next();

        self.make_memo_from(key, Expression::Continue { label: None })
    }

    // BreakExpression ::= `break`
    fn parse_break_expression(&mut self) -> ParseResult {
        let key = self.make_key("BreakExpression");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        // `break`
        if !matches!(self.tokens.get_keyword(), Some(MusubuKeyword::Break)) {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        }
        self.tokens.next();

        self.make_memo_from(
            key,
            Expression::Break {
                label: None,
                expression: None,
            },
        )
    }

    // ReturnExpression ::= `return` Expression?
    fn parse_return_expression(&mut self) -> ParseResult {
        let key = self.make_key("ReturnExpression");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        if !matches!(self.tokens.get_keyword(), Some(MusubuKeyword::Return)) {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        }
        self.tokens.next();

        let expr = self
            .option(Self::parse_expression)
            .and_then(|memo| memo.get_node())
            .and_then(|component| {
                let ASTNode::Expression(e) = component.as_ref() else {
                    return None;
                };
                Some(e.clone())
            });

        self.make_memo_from(key, Expression::Return(expr))
    }

    // ExpressionWithBlock ::= BlockExpression
    //                       | LoopExpression
    //                       | IfExpression
    //                       | IfLetExpression
    //                       | MatchExpression
    fn parse_expression_with_block(&mut self) -> ParseResult {
        let key = self.make_key("ExpressionWithBlock");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        self.or(vec![
            Self::parse_block_expression,
            Self::parse_loop_expression,
            Self::parse_if_expression,
        ])
    }

    // BlockExpression ::= `{` Statements? `}`
    fn parse_block_expression(&mut self) -> ParseResult {
        let key = self.make_key("BlockExpression");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        // `{`
        let Some(MusubuOperator::LeftBrace) = self.tokens.get_operator() else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };
        self.tokens.next();

        // Statements?
        let statements = self
            .option(Self::parse_statements)
            .and_then(|memo| memo.get_node())
            .and_then(|node| {
                let ASTNode::Statements(statements) = node.as_ref().clone() else {
                    unreachable!()
                };
                Some(statements)
            })
            .unwrap_or_default();

        // `}`
        let Some(MusubuOperator::RightBrace) = self.tokens.get_operator() else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };
        self.tokens.next();

        self.make_memo_from(key, Expression::Block(statements))
    }

    // LoopExpression ::= LoopLabel? (
    //                    InfiniteLoopExpression
    //                  | PredicateLoopExpression
    //                  | PredicatePatternLoopExpression
    //                  | IteratorLoopExpression
    //                  | LabelBlockExpression
    //                   )
    fn parse_loop_expression(&mut self) -> ParseResult {
        let key = self.make_key("LoopExpression");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        let Ok(result) = self.or(vec![
            Self::parse_infinite_loop_expression,
            Self::parse_predicate_loop_expression,
            Self::parse_iterator_loop_expression,
        ]) else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };
        let Some(node) = result.get_node().as_deref().cloned() else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };
        let ASTNode::Loop(loop_expr) = node else {
            unreachable!();
        };

        self.make_memo_from(key, Expression::Loop(loop_expr))
    }

    // InfiniteLoopExpression ::= `loop` BlockExpression
    fn parse_infinite_loop_expression(&mut self) -> ParseResult {
        let key = self.make_key("InfiniteLoopExpression");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        // `loop`
        if !matches!(self.tokens.get_keyword(), Some(MusubuKeyword::Loop)) {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        }
        self.tokens.next();

        // BlockExpression
        let Ok(result) = self.parse_block_expression() else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };
        let Some(node) = result.get_node() else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };
        let ASTNode::Expression(body) = node.as_ref().clone() else {
            unreachable!();
        };

        self.make_memo_from(key, LoopExpr::Loop { body })
    }

    // PredicateLoopExpression ::= `while` Expression BlockExpression
    fn parse_predicate_loop_expression(&mut self) -> ParseResult {
        let key = self.make_key("PredicateLoopExpression");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        // `while`
        if !matches!(self.tokens.get_keyword(), Some(MusubuKeyword::While)) {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        }
        self.tokens.next();

        // Expression
        let Ok(result) = self.parse_expression() else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };
        let Some(node) = result.get_node() else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };
        let ASTNode::Expression(condition) = node.as_ref().clone() else {
            unreachable!();
        };

        // BlockExpression
        let Ok(result) = self.parse_block_expression() else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };
        let Some(node) = result.get_node() else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };
        let ASTNode::Expression(body) = node.as_ref().clone() else {
            unreachable!();
        };

        self.make_memo_from(key, LoopExpr::While { body, condition })
    }

    // IteratorLoopExpression ::= `for` Pattern `in` BlockExpression
    fn parse_iterator_loop_expression(&mut self) -> ParseResult {
        let key = self.make_key("IteratorLoopExpression");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        // `for`
        if !matches!(self.tokens.get_keyword(), Some(MusubuKeyword::For)) {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        }
        self.tokens.next();

        // Pattern
        let Ok(ASTNode::Pattern(pattern)) = self.get_node(Self::parse_pattern).as_deref().cloned()
        else {
            unreachable!();
        };

        // `in`
        if !matches!(self.tokens.get_keyword(), Some(MusubuKeyword::In)) {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        }
        self.tokens.next();

        // Expression
        // except struct expression
        let Ok(iterator) = self.get_expr(Self::parse_expression) else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };

        // Block
        let Ok(body) = self.get_expr(Self::parse_block_expression) else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };

        self.make_memo_from(
            key,
            LoopExpr::For {
                pattern: pattern.clone(),
                iterator,
                body,
            },
        )
    }

    // IfExpression ::= `if` Expression BlockExpression ( `else` ( BlockExpression | IfExpression | IfLetExpression ) )?
    fn parse_if_expression(&mut self) -> ParseResult {
        let key = self.make_key("IfExpression");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        // `if`
        if !matches!(self.tokens.get_keyword(), Some(MusubuKeyword::If)) {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        }
        self.tokens.next();

        // Expression
        // 条件式
        let Ok(condition) = self.get_expr(Self::parse_expression) else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };

        // BlockExpression
        // thenブロック
        let Ok(then_body) = self.get_expr(Self::parse_block_expression) else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };

        // else
        let else_body = if matches!(self.tokens.get_keyword(), Some(MusubuKeyword::Else)) {
            self.tokens.next();

            // elseブロック
            if let Ok(ASTNode::Expression(expr)) = self
                .get_node(Self::parse_block_expression)
                .as_deref()
                .cloned()
            {
                Some(expr)
            } else if let Ok(ASTNode::Expression(expr)) =
                self.get_node(Self::parse_if_expression).as_deref().cloned()
            {
                Some(expr)
            } else {
                return self.make_memo_from_result(key, Err(ParseError::UnexpectedAST));
            }
        } else {
            None
        };

        self.make_memo_from(
            key,
            Expression::If {
                condition,
                then_body,
                else_body,
            },
        )
    }

    // Statements ::= Statement+
    //              | Statement+ ExpressionWithoutBlock
    //              | ExpressionWithoutBlock
    fn parse_statements(&mut self) -> ParseResult {
        let key = self.make_key("Statements");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        // Statement+
        let mut statements = vec![];
        if let Ok(list) = self.one_or_more(Self::parse_statement) {
            for memo in list {
                let Some(node) = memo.get_node() else {
                    return self.make_memo_from_result(key, Err(ParseError::NotMatch));
                };
                let ASTNode::Statement(statement) = node.as_ref() else {
                    unreachable!();
                };
                statements.push(statement.clone());
            }
        }

        // ExpressionWithoutBlock
        if let Some(node) = self
            .option(Self::parse_expression_without_block)
            .map(|memo| memo.get_node())
            .flatten()
        {
            let ASTNode::Expression(expr) = node.as_ref().clone() else {
                unreachable!();
            };
            let span = expr.span;
            statements.push(Spanned {
                node: Statement::Expression(expr),
                span,
            });
        }

        if statements.is_empty() {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        }

        self.make_memo_from_node(key, Rc::new(ASTNode::Statements(statements)))
    }

    // Statement ::= `;` | Item | LetStatement | ExpressionStatement
    fn parse_statement(&mut self) -> ParseResult {
        let key = self.make_key("Statement");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        // `;` | Item | LetStatement | ExpressionStatement
        let result = self.or(vec![
            |parser: &mut Self| -> ParseResult {
                let key = parser.make_key(";");
                let Some(MusubuOperator::Semicolon) = parser.tokens.get_operator() else {
                    return Err(ParseError::NotMatch);
                };
                parser.tokens.next();
                parser.make_memo_from(key, Statement::Semicolon)
            },
            Self::parse_item,
            Self::parse_let_statement,
            Self::parse_expression_statement,
        ]);
        let Ok(Some(node)) = result.map(|memo| memo.get_node().as_deref().cloned()) else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };
        let statement = match node {
            ASTNode::Item { item, .. } => Statement::Item(item),
            ASTNode::Expression(expr) => Statement::Expression(expr),
            ASTNode::Statement(statement) => statement.node,
            _ => unreachable!(),
        };

        self.make_memo_from(key, statement)
    }

    // LetStatement ::= `let` PatternNoTopAlt ( `:` Type )? (`=` Expression )? `;`
    fn parse_let_statement(&mut self) -> ParseResult {
        let key = self.make_key("LetStatement");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        // `let`
        if !matches!(self.tokens.get_keyword(), Some(MusubuKeyword::Let)) {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        }
        self.tokens.next();

        // PatternNoTopAlt
        let Ok(pattern_kind) = self.get_node(Self::parse_pattern_no_top_alt) else {
            return self.make_memo_from_result(key, Err(ParseError::UnexpectedAST));
        };
        let ASTNode::Pattern(pattern) = pattern_kind.as_ref() else {
            return self.make_memo_from_result(key, Err(ParseError::UnexpectedAST));
        };

        // ( `:` Type )?
        let variable_type = if let Some(MusubuOperator::Colon) = self.tokens.get_operator() {
            self.tokens.next();
            self.get_node(Self::parse_type).ok().and_then(|k| {
                let ASTNode::Type(t) = k.as_ref() else {
                    unreachable!();
                };
                Some(t.clone())
            })
        } else {
            None
        };

        // (`=` Expression )?
        let initializer = if let Some(AssignOperator::Assign) = self.tokens.get_assign_operator() {
            self.tokens.next();
            self.get_node(Self::parse_expression).ok().and_then(|node| {
                let ASTNode::Expression(expr) = node.as_ref() else {
                    unreachable!();
                };
                Some(expr.clone())
            })
        } else {
            None
        };

        if self.tokens.get_operator() != Some(&MusubuOperator::Semicolon) {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        }
        self.tokens.next();

        self.make_memo_from(
            key,
            Statement::Let {
                name: pattern.clone(),
                variable_type,
                initializer,
                label: None,
            },
        )
    }

    // ExpressionStatement ::= ExpressionWithoutBlock `;` | ExpressionWithBlock `;`?
    fn parse_expression_statement(&mut self) -> ParseResult {
        let key = self.make_key("ExpressionStatement");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        };

        let result = self.or(vec![
            // ExpressionWithoutBlock `;`
            |parser: &mut Self| -> ParseResult {
                let result = parser.parse_expression_without_block()?;
                let Some(MusubuOperator::Semicolon) = parser.tokens.get_operator() else {
                    return Err(ParseError::NotMatch);
                };
                parser.tokens.next();
                Ok(result)
            },
            // ExpressionWithBlock `;`?
            |parser: &mut Self| -> ParseResult {
                let result = parser.parse_expression_with_block()?;
                if let Some(MusubuOperator::Semicolon) = parser.tokens.get_operator() {
                    parser.tokens.next();
                };
                Ok(result)
            },
        ]);

        let Ok(memo) = result else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };
        let Some(node) = memo.get_node() else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };
        let ASTNode::Expression(expr) = node.as_ref().clone() else {
            unreachable!("");
        };

        self.make_memo_from(key, Statement::Expression(expr))
    }

    // Type ::= TypeNoBounds
    fn parse_type(&mut self) -> ParseResult {
        let key = self.make_key("Type");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        let Ok(kind) = self.get_node(Self::parse_type_no_bounds) else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };
        let ASTNode::Type(t) = kind.as_ref() else {
            unreachable!();
        };

        self.make_memo_from_node(key, Rc::new(ASTNode::Type(t.clone())))
    }

    // TypeNoBounds ::= ParenthesizedType
    //                | TypePath
    //                | TupleType
    //                | ReferenceType
    //                | ArrayType
    //                | QualifiedPathInType
    //                | BareFunctionType
    fn parse_type_no_bounds(&mut self) -> ParseResult {
        let key = self.make_key("TypeNoBounds");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        let result = self.or(vec![Self::parse_type_path]);
        self.make_memo_from_result(key, result)
    }

    // TypePath ::= `::`? TypePathSegment (`::` TypePathSegment)*
    fn parse_type_path(&mut self) -> ParseResult {
        let key = self.make_key("TypePath");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        // `::`?
        if let Some(MusubuOperator::Path) = self.tokens.get_operator() {
            self.tokens.next();
        }

        // TypePathSegment
        let Ok(first) = self.get_node(Self::parse_type_path_segment) else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };

        // (`::` TypePathSegment)*
        let segments = self.zero_or_more(|parser: &mut Self| -> ParseResult {
            let Some(MusubuOperator::Path) = parser.tokens.get_operator() else {
                return Err(ParseError::NotMatch);
            };
            parser.tokens.next();
            parser.parse_type_path_segment()
        });
        let mut segments = segments
            .into_iter()
            .map(|memo| {
                let Some(node) = memo.get_node() else {
                    unreachable!();
                };
                node
            })
            .collect::<Vec<_>>();
        segments.insert(0, first);

        // 変換
        let segments = segments
            .into_iter()
            .map(|seg| {
                let ASTNode::PathSegment(seg) = seg.as_ref() else {
                    unreachable!();
                };
                seg.clone()
            })
            .collect::<Vec<_>>();

        let span = self.make_span(&key);
        self.make_memo_from(
            key,
            TypeKind::PathType(Spanned {
                node: Box::new(Path { segments }),
                span,
            }),
        )
    }

    // TypePathSegment ::= PathIdentSegment (`::`? (GenericArgs | TypePathFn))?
    fn parse_type_path_segment(&mut self) -> ParseResult {
        let key = self.make_key("TypePathSegment");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        // PathIdentSegment
        let Ok(segment) = self.get_node(Self::parse_path_ident_segment) else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };
        let ASTNode::Segment(name) = segment.as_ref() else {
            unreachable!();
        };

        // (`::`? (GenericArgs | TypePathFn))?
        let arguments = self
            .option(|parser: &mut Self| -> ParseResult {
                if let Some(MusubuOperator::Path) = parser.tokens.get_operator() {
                    parser.tokens.next();
                }
                parser.or(vec![Self::parse_generic_args, Self::parse_type_path_fn])
            })
            .and_then(|memo| memo.get_node())
            .and_then(|node| {
                let ASTNode::Arguments(args) = node.as_ref() else {
                    unreachable!();
                };
                Some(args.clone())
            })
            .unwrap_or_default();

        self.make_memo_from(
            key,
            PathSegment {
                ident: name.node.clone(),
                arguments,
            },
        )
    }

    // TypePathFn ::= `(` TypePathFnInputs? `)` (`->` TypeNoBounds)?
    fn parse_type_path_fn(&mut self) -> ParseResult {
        let key = self.make_key("TypePathFn");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        let Some(MusubuOperator::LeftParenthesis) = self.tokens.get_operator() else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };
        self.tokens.next();

        let params = self
            .option(Self::parse_type_path_inputs)
            .and_then(|memo| memo.get_node())
            .and_then(|kind| {
                if let ASTNode::Arguments(types) = kind.as_ref() {
                    Some(types.clone())
                } else {
                    None
                }
            })
            .unwrap_or_default();

        let Some(MusubuOperator::RightParenthesis) = self.tokens.get_operator() else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };
        self.tokens.next();

        let return_type = self
            .option(|parser| {
                let Some(MusubuOperator::RightArrow) = parser.tokens.get_operator() else {
                    return Err(ParseError::NotMatch);
                };
                parser.tokens.next();
                parser.parse_type_no_bounds()
            })
            .and_then(|memo| memo.get_node())
            .and_then(|kind| {
                let ASTNode::Type(type_kind) = kind.as_ref() else {
                    return None;
                };
                Some(Spanned {
                    node: Box::new(type_kind.node.clone()),
                    span: type_kind.span,
                })
            })
            .unwrap_or(Spanned {
                node: Box::new(TypeKind::Primitive(PrimitiveType::Unit)),
                span: Span::default(),
            });

        self.make_memo_from(
            key,
            TypeKind::Function {
                params,
                return_type,
            },
        )
    }

    // TypePathFnInputs ::= Type (`,` Type)* `,`?
    fn parse_type_path_inputs(&mut self) -> ParseResult {
        let key = self.make_key("TypePathFnInputs");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        // Type
        let Ok(first) = self.get_node(Self::parse_type) else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };

        let Some(mut inputs) = self
            .zero_or_more(|parser: &mut Self| -> ParseResult {
                let Some(MusubuOperator::Comma) = parser.tokens.get_operator() else {
                    return Err(ParseError::NotMatch);
                };
                parser.tokens.next();
                parser.parse_type()
            })
            .into_iter()
            .map(|memo| memo.get_node())
            .collect::<Option<Vec<_>>>()
        else {
            return self.make_memo_from_result(key, Err(ParseError::UnexpectedAST));
        };

        inputs.insert(0, first);

        // 変換
        let types = inputs
            .into_iter()
            .map(|kind| {
                let ASTNode::Type(t) = kind.as_ref() else {
                    unreachable!();
                };
                t.clone()
            })
            .collect::<Vec<_>>();

        self.make_memo_from_node(key, Rc::new(ASTNode::Arguments(types)))
    }

    // Pattern ::= `|`? PatternNoTopAlt ( `|` PatternNoTopAlt )*
    fn parse_pattern(&mut self) -> ParseResult {
        let key = self.make_key("Pattern");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        if let Some(BinaryOperator::Or) = self.tokens.get_binary_operator() {
            self.tokens.next();
        }

        let Ok(first) = self
            .get_node(Self::parse_pattern_no_top_alt)
            .as_deref()
            .cloned()
        else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };
        let ASTNode::Pattern(first) = first else {
            unreachable!();
        };

        // ( `|` PatternNoTopAlt )*
        let patterns = self.zero_or_more(|parser: &mut Self| -> ParseResult {
            let Some(BinaryOperator::Or) = parser.tokens.get_binary_operator() else {
                return Err(ParseError::NotMatch);
            };
            parser.tokens.next();
            parser.parse_pattern_no_top_alt()
        });

        // 結合
        let pattern = if patterns.is_empty() {
            first
        } else {
            let mut patterns = patterns
                .into_iter()
                .map(|memo| {
                    let Some(node) = memo.get_node() else {
                        unreachable!();
                    };
                    let ASTNode::Pattern(pattern) = node.as_ref().clone() else {
                        unreachable!()
                    };
                    pattern
                })
                .collect::<Vec<_>>();

            patterns.insert(0, first);

            let span = self.make_span(&key);
            Spanned {
                node: Pattern::Multiply(patterns),
                span,
            }
        };

        self.make_memo_from_node(key, Rc::new(ASTNode::Pattern(pattern)))
    }

    // PatternNoTopAlt ::= PatternWithoutRange | RangePattern
    fn parse_pattern_no_top_alt(&mut self) -> ParseResult {
        let key = self.make_key("PatternNoTopAlt");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        let result = self.parse_pattern_without_range();
        self.make_memo_from_result(key, result)
    }

    // PatternWithoutRange ::= LiteralPattern
    //                       | IDENTIFIERPattern
    //
    fn parse_pattern_without_range(&mut self) -> ParseResult {
        let key = self.make_key("PatternWithoutRange");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }
        let result = self.or(vec![
            Self::parse_literal_pattern,
            Self::parse_identifier_pattern,
        ]);
        self.make_memo_from_result(key, result)
    }

    // LiteralPattern ::= `true`
    //                  | `false`
    //                  | CHAR_LITERAL
    //                  | STRING_LITERAL
    //                  | INTEGER_LITERAL
    //                  | FLOAT_LITERAL
    fn parse_literal_pattern(&mut self) -> ParseResult {
        let key = self.make_key("LiteralPattern");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        if matches!(self.tokens.get_keyword(), Some(MusubuKeyword::True)) {
            self.tokens.next();
            return self.make_memo_from(key, Pattern::Literal(Literal::Bool(true)));
        }
        if matches!(self.tokens.get_keyword(), Some(MusubuKeyword::False)) {
            self.tokens.next();
            return self.make_memo_from(key, Pattern::Literal(Literal::Bool(false)));
        }

        if let Some(literal) = self.tokens.get_literal().cloned() {
            self.tokens.next();
            return match literal.to_literal() {
                Ok(literal) => self.make_memo_from(key, Pattern::Literal(literal)),
                Err(err) => self.make_memo_from_result(key, Err(err)),
            };
        }

        Err(ParseError::NotMatch)
    }

    // IDENTIFIERPattern ::= `ref`? `mut`? IDENTIFIER (`@` PatternNoTopAlt )?
    fn parse_identifier_pattern(&mut self) -> ParseResult {
        let key = self.make_key("IDENTIFIERPattern");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        let is_ref = matches!(self.tokens.get_keyword(), Some(MusubuKeyword::Ref));
        if is_ref {
            self.tokens.next();
        }

        let is_mut = matches!(self.tokens.get_keyword(), Some(MusubuKeyword::Mut));
        if is_mut {
            self.tokens.next();
        }

        let Some(name) = self.tokens.get_identifier() else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };
        let name = name.to_string();
        self.tokens.next();

        let subpattern = if let Some(MusubuOperator::At) = self.tokens.get_operator() {
            self.tokens.next();

            self.get_node(Self::parse_pattern_no_top_alt)
                .ok()
                .and_then(|k| {
                    if let ASTNode::Pattern(p) = k.as_ref() {
                        Some(Box::new(p.clone()))
                    } else {
                        None
                    }
                })
        } else {
            None
        };

        self.make_memo_from(
            key,
            Pattern::Identifier {
                ident: name,
                reference: is_ref,
                mutable: is_mut,
            },
        )
    }

    // CallParams ::= Expression ( `,` Expression )* `,`?
    fn parse_call_params(&mut self) -> ParseResult {
        let key = self.make_key("CallParams");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        // Expression
        let Ok(expr) = self.get_expr(Self::parse_expression) else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };

        // ( `,` Expression )*
        let params = self.zero_or_more(|parser: &mut Self| -> ParseResult {
            let Some(MusubuOperator::Comma) = parser.tokens.get_operator() else {
                return Err(ParseError::NotMatch);
            };
            parser.tokens.next();
            parser.parse_expression()
        });

        // 変換
        let params = params
            .into_iter()
            .map(|memo| {
                let Some(node) = memo.get_node().as_deref().cloned() else {
                    return Err(ParseError::NotMatch);
                };
                let ASTNode::Expression(expr) = node else {
                    unreachable!();
                };
                Ok(expr)
            })
            .collect::<Result<Vec<_>, _>>();
        let Ok(mut params) = params else {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        };

        params.insert(0, expr);

        if let Some(MusubuOperator::Comma) = self.tokens.get_operator() {
            self.tokens.next();
        };

        let node = ASTNode::CallParams(params);
        self.make_memo_from_node(key, Rc::new(node))
    }

    // Pratt Parsing
    // メモ化はしない
    fn pratt_parse(&mut self, min_bp: u16) -> ParseResult {
        let key = self.make_key("<Pratt>");
        self.bp_stack.push(min_bp);

        // 前置演算子or式
        let Ok(mut lhs) = self.expr_or_prefix_op() else {
            return Err(ParseError::NotMatch);
        };

        // 演算子に応じた判定
        loop {
            let Some(op) = self.tokens.get_operator().cloned() else {
                break;
            };

            // 後置演算子
            if let Some(left_bp) = op.get_postfix_binding_power() {
                if left_bp < min_bp {
                    break;
                }
                self.tokens.next();

                lhs = if let Some(pair) = op.counterpart_of() {
                    // 中身だけを判断
                    let rhs = self.postfix_pair_inside(&op)?;
                    if Some(&pair) != self.tokens.get_operator() {
                        return Err(ParseError::NotMatch);
                    }
                    self.tokens.next();

                    let span = self.make_span(&key);
                    make_ast_from_operator(span, &op, Some(lhs), None, Some(rhs))?
                } else {
                    lhs
                };
                continue;
            }

            // 中置演算子
            if let Some((left_bp, right_bp)) = op.get_infix_binding_power() {
                if left_bp < min_bp {
                    break;
                }
                self.tokens.next();

                lhs = if let Some(pair) = op.counterpart_of() {
                    let Some(mhs) = self.pratt_parse(0)?.get_node() else {
                        return Err(ParseError::UnexpectedAST);
                    };
                    if Some(&pair) != self.tokens.get_operator() {
                        return Err(ParseError::NotMatch);
                    }
                    self.tokens.next();

                    let Some(rhs) = self.pratt_parse(right_bp)?.get_node() else {
                        return Err(ParseError::UnexpectedAST);
                    };

                    let span = self.make_span(&key);
                    make_ast_from_operator(span, &op, Some(lhs), Some(mhs), Some(rhs))?
                } else {
                    let Some(rhs) = self.pratt_parse(right_bp)?.get_node() else {
                        return Err(ParseError::UnexpectedAST);
                    };
                    let span = self.make_span(&key);

                    make_ast_from_operator(span, &op, Some(lhs), None, Some(rhs))?
                };

                continue;
            }

            break;
        }

        self.bp_stack.pop();
        Ok(MemoResult::from_node(lhs, self.tokens.get_position()))
    }

    fn expr_or_prefix_op(&mut self) -> Result<Rc<ASTNode>, ParseError> {
        // 演算子以外はExpressionとしてパース
        let Some(op) = self.tokens.get_operator() else {
            return self
                .parse_expression()?
                .get_node()
                .ok_or(ParseError::NotMatch);
        };

        // ペア
        if let Some(pair) = op.counterpart_of() {
            self.tokens.next();
            let lhs = self.pratt_parse(0)?.get_node();
            if self.tokens.get_operator() == Some(&pair) {
                return Err(ParseError::NotMatch);
            }
            self.tokens.next();
            return lhs.ok_or(ParseError::NotMatch);
        }

        // その他演算子
        let Some(r_bp) = op.get_prefix_binding_power() else {
            return Err(ParseError::UnexpectedOperator);
        };
        self.tokens.next();
        let Some(lhs) = self.pratt_parse(r_bp)?.get_node() else {
            return Err(ParseError::UnexpectedOperator);
        };

        Ok(lhs)
    }

    fn postfix_pair_inside(&mut self, left_op: &MusubuOperator) -> Result<Rc<ASTNode>, ParseError> {
        match left_op {
            MusubuOperator::LeftParenthesis => self
                .parse_call_params()?
                .get_node()
                .ok_or(ParseError::NotMatch),

            _ => self.pratt_parse(0)?.get_node().ok_or(ParseError::NotMatch),
        }
    }

    // 以下ユーティリティ

    fn get_expr<F>(&mut self, function: F) -> Result<SpannedBox<Expression>, ParseError>
    where
        F: FnOnce(&mut Self) -> ParseResult,
    {
        let Ok(result) = function(self) else {
            return Err(ParseError::NotMatch);
        };
        let Some(node) = result.get_node().as_deref().cloned() else {
            return Err(ParseError::NotMatch);
        };
        let ASTNode::Expression(expr) = node else {
            unreachable!();
        };

        Ok(expr)
    }

    fn get_node<F>(&mut self, function: F) -> Result<Rc<ASTNode>, ParseError>
    where
        F: FnOnce(&mut Self) -> ParseResult,
    {
        let MemoResult::Match { memo, .. } = function(self)? else {
            return Err(ParseError::UnexpectedAST);
        };

        let ast = match memo {
            Memo::ASTNode(ast) => ast,
        };
        Ok(ast)
    }

    // |
    fn or<F>(&mut self, functions: Vec<F>) -> ParseResult
    where
        F: FnOnce(&mut Self) -> ParseResult,
    {
        let position = self.tokens.get_position();
        for function in functions {
            let Ok(res) = function(self) else {
                self.tokens.set_position(position);
                continue;
            };
            return Ok(res);
        }

        Err(ParseError::NotMatch)
    }

    // ?
    fn option<F>(&mut self, function: F) -> Option<MemoResult>
    where
        F: FnOnce(&mut Self) -> ParseResult,
    {
        let position = self.tokens.get_position();
        function(self)
            .map_err(|err| {
                self.tokens.set_position(position);
                err
            })
            .ok()
    }

    // +
    fn one_or_more<F>(&mut self, function: F) -> Result<Vec<MemoResult>, ParseError>
    where
        F: FnOnce(&mut Self) -> ParseResult + Copy,
    {
        let mut vec = vec![function(self)?];
        while let Some(result) = self.option(function) {
            vec.push(result);
        }
        Ok(vec)
    }

    // *
    fn zero_or_more<F>(&mut self, function: F) -> Vec<MemoResult>
    where
        F: FnOnce(&mut Self) -> ParseResult + Copy,
    {
        let mut vec = vec![];
        while let Some(result) = self.option(function) {
            vec.push(result);
        }
        vec
    }

    fn make_key(&self, rule: &'a str) -> MemoKey<'a> {
        MemoKey {
            rule,
            position: self.tokens.get_position(),
        }
    }

    fn get_memo(&mut self, key: &MemoKey<'a>) -> Option<MemoResult> {
        let memo = self.get_memo_uncheck(key)?;
        if matches!(memo, MemoResult::Pending) {
            return None; // 左再帰を起こしている
        }

        Some(memo)
    }

    fn get_memo_uncheck(&mut self, key: &MemoKey<'a>) -> Option<MemoResult> {
        #[cfg(test)]
        if true {
            println!(
                "in({:?}): {:?} {key:?} {:?}",
                self.memo.contains_key(key),
                self.bp_stack.last(),
                self.tokens.get()
            );
        }

        // メモが存在した場合はその内容を返す
        if let Some(memo) = self.memo.get(key) {
            match memo {
                MemoResult::Match { next_position, .. } => {
                    self.tokens.set_position(*next_position);
                }
                MemoResult::Pending => (),
                MemoResult::NotMatch => (),
            }

            return Some(memo.clone());
        }

        if self.max_read_position < self.tokens.get_position() {
            self.last_fail_rule = Some(key.rule);
        }

        self.memo.insert(key.clone(), MemoResult::Pending);

        None
    }

    fn make_span(&self, key: &MemoKey<'a>) -> Span {
        let start = self
            .tokens
            .get_from_index(key.position)
            .map(|token| token.position)
            .unwrap_or(0);
        let end = self
            .tokens
            .get()
            .map(|token| token.position)
            .unwrap_or(start);
        Span {
            file_id: 0, // 仮
            start: start as u32,
            end: end as u32,
        }
    }

    fn make_memo_from<N>(&mut self, key: MemoKey<'a>, node_kind: N) -> ParseResult
    where
        N: NodeMaker,
    {
        let node = node_kind.make_node(self.make_span(&key));
        let memo = MemoResult::Match {
            memo: Memo::ASTNode(Rc::new(node)),
            next_position: self.tokens.get_position(),
        };
        self.make_memo_from_result(key, Ok(memo))
    }

    fn make_memo_from_node(&mut self, key: MemoKey<'a>, node: Rc<ASTNode>) -> ParseResult {
        let memo = MemoResult::Match {
            memo: Memo::ASTNode(node),
            next_position: self.tokens.get_position(),
        };
        self.make_memo_from_result(key, Ok(memo))
    }

    fn make_memo_from_result(&mut self, key: MemoKey<'a>, result: ParseResult) -> ParseResult {
        #[cfg(test)]
        if true {
            println!(
                "out({:?}): {:?} {key:?}, {:?}",
                result.is_ok(),
                self.bp_stack.last(),
                self.tokens.get()
            );
        }

        let memo = match &result {
            Ok(memo) => {
                let pos = self.tokens.get_position();
                if self.max_read_position < pos {
                    self.max_read_position = pos;
                }

                memo.clone()
            }
            Err(_) => {
                self.tokens.set_position(key.position);
                MemoResult::NotMatch
            }
        };
        self.memo.insert(key, memo);

        result
    }
}

fn make_ast_from_operator(
    span: Span,
    op: &MusubuOperator,
    lhs: Option<Rc<ASTNode>>,
    mhs: Option<Rc<ASTNode>>,
    rhs: Option<Rc<ASTNode>>,
) -> Result<Rc<ASTNode>, ParseError> {
    // ()など特殊な文
    match op {
        MusubuOperator::LeftParenthesis => return make_brackets_ast(span, lhs, mhs, rhs),
        _ => (),
    };

    let convert = |node: Option<Rc<ASTNode>>| -> Option<_> {
        let ASTNode::Expression(expr) = node?.as_ref().clone() else {
            return None;
        };
        Some(expr)
    };
    let lhs = convert(lhs);
    let mhs = convert(mhs);
    let rhs = convert(rhs);

    match [lhs, mhs, rhs] {
        [Some(lhs), None, None] => {
            unimplemented!()
        }
        [None, None, Some(rhs)] => {
            unimplemented!()
        }
        [Some(lhs), None, Some(rhs)] => match op {
            MusubuOperator::Binary(op) => make_binary_op_ast(span, op, lhs, rhs),
            MusubuOperator::Assign(op) => make_assign_op_ast(span, op, lhs, rhs),
            MusubuOperator::Comparison(op) => make_comparison_op_ast(span, op, lhs, rhs),
            MusubuOperator::Logical(op) => make_logical_op_ast(span, op, lhs, rhs),
            _ => Err(ParseError::NotMatch),
        },
        [Some(lhs), Some(mhs), Some(rhs)] => {
            unimplemented!()
        }
        _ => unreachable!(),
    }
}

fn make_binary_op_ast(
    span: Span,
    operator: &BinaryOperator,
    left: SpannedBox<Expression>,
    right: SpannedBox<Expression>,
) -> Result<Rc<ASTNode>, ParseError> {
    Ok(Rc::new(
        Expression::Binary {
            operator: operator.clone(),
            left,
            right,
        }
        .make_node(span),
    ))
}

fn make_assign_op_ast(
    span: Span,
    operator: &AssignOperator,
    left: SpannedBox<Expression>,
    right: SpannedBox<Expression>,
) -> Result<Rc<ASTNode>, ParseError> {
    Ok(Rc::new(
        Expression::Assign {
            operator: operator.clone(),
            left,
            right,
        }
        .make_node(span),
    ))
}

fn make_comparison_op_ast(
    span: Span,
    operator: &ComparisonOperator,
    left: SpannedBox<Expression>,
    right: SpannedBox<Expression>,
) -> Result<Rc<ASTNode>, ParseError> {
    Ok(Rc::new(
        Expression::Comparison {
            operator: operator.clone(),
            left,
            right,
        }
        .make_node(span),
    ))
}

fn make_logical_op_ast(
    span: Span,
    operator: &LogicalOperator,
    left: SpannedBox<Expression>,
    right: SpannedBox<Expression>,
) -> Result<Rc<ASTNode>, ParseError> {
    Ok(Rc::new(
        Expression::Logical {
            operator: operator.clone(),
            left,
            right,
        }
        .make_node(span),
    ))
}

fn make_brackets_ast(
    span: Span,
    lhs: Option<Rc<ASTNode>>,
    mhs: Option<Rc<ASTNode>>,
    rhs: Option<Rc<ASTNode>>,
) -> Result<Rc<ASTNode>, ParseError> {
    let lhs = lhs.as_deref().cloned();
    let mhs = mhs.as_deref().cloned();
    let rhs = rhs.as_deref().cloned();

    let node = match [lhs, mhs, rhs] {
        [
            Some(ASTNode::Expression(function)),
            None,
            Some(ASTNode::CallParams(arguments)),
        ] => Expression::Call {
            function,
            arguments,
        }
        .make_node(span),
        _ => unreachable!(),
    };

    Ok(Rc::new(node))
}
