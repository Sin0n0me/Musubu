use crate::{
    errors::ParseError,
    lexer::{musubu_keywords::MusubuKeyword, token::MusubuOperator},
    parser::packrat::{PackratAndPrattParser, ParseResult},
};
use musubu_ast::{ASTNode, FunctionParam, Item, Visibility};
use musubu_span::Spanned;
use std::rc::Rc;

impl<'a> PackratAndPrattParser<'a> {
    // Function ::= `fn` IDENTIFIER `(` elseFunctionParameters? `)` FunctionReturnType? ( BlockExpression | `;` )
    pub(in crate::parser) fn parse_function(&mut self) -> ParseResult {
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
}
