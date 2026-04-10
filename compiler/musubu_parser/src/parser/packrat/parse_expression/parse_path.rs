use crate::{
    errors::ParseError,
    lexer::token::MusubuOperator,
    parser::packrat::{MemoResult, PackratAndPrattParser, ParseResult},
};
use musubu_ast::{ASTNode, AssignOperator, Path, PathSegment};
use musubu_primitive::ComparisonOperator;
use musubu_span::Spanned;
use std::rc::Rc;

impl<'a> PackratAndPrattParser<'a> {
    // PathInExpression ::= `::`? PathExprSegment ( `::` PathExprSegment )*
    pub(super) fn parse_path_in_expression(&mut self) -> ParseResult {
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
    pub(in crate::parser) fn parse_path_ident_segment(&mut self) -> ParseResult {
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
    pub(in crate::parser) fn parse_generic_args(&mut self) -> ParseResult {
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
    pub(super) fn parse_qualified_path_in_expression(&mut self) -> ParseResult {
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
}
