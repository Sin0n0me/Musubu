use crate::{
    errors::ParseError,
    lexer::{musubu_keywords::MusubuKeyword, token::MusubuOperator},
    parser::packrat::{PackratAndPrattParser, ParseResult},
};
use musubu_ast::{ASTNode, Item, StructField, Visibility};
use musubu_span::Spanned;
use std::rc::Rc;

impl<'a> PackratAndPrattParser<'a> {
    // Struct ::= StructStruct | TupleStruct
    pub(in crate::parser) fn parse_struct(&mut self) -> ParseResult {
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
    pub(in crate::parser) fn parse_struct_fields(&mut self) -> ParseResult {
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
}
