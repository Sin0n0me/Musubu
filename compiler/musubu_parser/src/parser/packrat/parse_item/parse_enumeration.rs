use crate::{
    errors::ParseError,
    lexer::{musubu_keywords::MusubuKeyword, token::MusubuOperator},
    parser::packrat::{PackratAndPrattParser, ParseResult},
};
use musubu_ast::{ASTNode, EnumItem, Item, Visibility};
use std::rc::Rc;

impl<'a> PackratAndPrattParser<'a> {
    // Enumeration ::= `enum` IDENTIFIER GenericParams? WhereClause? `{` EnumItems? `}`
    pub(in crate::parser) fn parse_enumeration(&mut self) -> ParseResult {
        let key = self.make_key("Enumeration");
        if let Some(memo) = self.get_memo(&key) {
            return Ok(memo);
        }

        // `enum`
        if !matches!(self.tokens.get_keyword(), Some(MusubuKeyword::Enum)) {
            return self.make_memo_from_result(key, Err(ParseError::NotMatch));
        }
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
}
