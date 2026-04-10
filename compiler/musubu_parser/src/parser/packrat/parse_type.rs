use crate::{
    errors::ParseError,
    lexer::token::MusubuOperator,
    parser::packrat::{PackratAndPrattParser, ParseResult},
};
use musubu_ast::{ASTNode, Path, PathSegment, TypeKind};
use musubu_primitive::PrimitiveType;
use musubu_span::{Span, Spanned};
use std::rc::Rc;

impl<'a> PackratAndPrattParser<'a> {
    // Type ::= TypeNoBounds
    pub(in crate::parser) fn parse_type(&mut self) -> ParseResult {
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
                let ASTNode::Arguments(types) = kind.as_ref() else {
                    unreachable!();
                };
                Some(types.clone())
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
}
