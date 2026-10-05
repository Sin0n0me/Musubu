mod parse_collection;
mod parse_expression;
mod parse_item;
mod parse_pattern;
mod parse_type;
mod pratt;

use crate::{TokenStream, errors::ParseError, lexer::token::BindingPower};
use alloc::rc::Rc;
use alloc::{vec, vec::Vec};
use core::hash::BuildHasherDefault;
use musubu_ast::*;
use musubu_span::*;
use twox_hash::XxHash64;

pub(crate) type Hasher = BuildHasherDefault<XxHash64>;
pub(crate) type IndexMap<K, V> = indexmap::IndexMap<K, V, Hasher>;

pub(crate) fn make_index_map<K, V>() -> IndexMap<K, V> {
    IndexMap::with_hasher(Hasher::default())
}

#[derive(Debug, Clone)]
pub(crate) enum Memo {
    ASTNode(Rc<ASTNode>),
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub(crate) struct MemoKey {
    rule: &'static str,
    position: usize,
}

#[derive(Debug, Clone)]
pub(crate) enum MemoResult {
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
pub(crate) struct PackratAndPrattParser {
    memo: IndexMap<MemoKey, MemoResult>,
    tokens: TokenStream,
    max_read_position: usize,
    last_fail_rule: Option<&'static str>,
    failure: Option<(usize, usize, ParseError)>,
    bp_stack: Vec<BindingPower>,
}

pub(crate) type ParseResult = Result<MemoResult, ParseError>;

// TODO マクロで共通処理を纏める
//
// 仕様上想定していない場合には unreachable! マクロを使用
// それ以外コンパイルエラーとして Err を返す
//
impl PackratAndPrattParser {
    pub fn new(tokens: TokenStream) -> Self {
        Self {
            memo: make_index_map(),
            tokens,
            max_read_position: 0,
            last_fail_rule: None,
            failure: None,
            bp_stack: Vec::new(),
        }
    }

    pub fn parse(&mut self) -> Result<Vec<Rc<ASTNode>>, ParseError> {
        self.memo.clear();

        let mut ast_items = Vec::new();
        while self.tokens.get().is_some() {
            self.failure = None;
            self.max_read_position = self.tokens.get_position();
            self.last_fail_rule = None;
            self.tokens.reset_furthest();
            let result = self
                .parse_item()
                .and_then(|memo| memo.get_node().ok_or(ParseError::UnexpectedAST));
            let item = result.map_err(|error| self.diagnostic(error))?;

            ast_items.push(item);
        }

        Ok(ast_items)
    }

    fn diagnostic(&mut self, error: ParseError) -> ParseError {
        let index = self.max_read_position.max(self.tokens.furthest());
        let furthest_start = self
            .tokens
            .get_from_index(index)
            .map(|token| token.position)
            .unwrap_or(self.tokens.source_end());
        let failure = self.failure.take().filter(|(start, _, error)| {
            *start >= furthest_start || !matches!(error, ParseError::Expected { .. })
        });
        let (start, end, error) = failure.unwrap_or_else(|| {
            let start = self
                .tokens
                .get_from_index(index)
                .map(|token| token.position)
                .unwrap_or(self.tokens.source_end());
            (
                start,
                self.tokens.token_end(index),
                match error {
                    ParseError::NotMatch | ParseError::UnexpectedAST => ParseError::Expected {
                        rule: self
                            .last_fail_rule
                            .unwrap_or("an item (fn, struct or enum)"),
                    },
                    error => error,
                },
            )
        });
        ParseError::Located {
            start,
            end,
            error: alloc::boxed::Box::new(error),
        }
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

    // ConstantItem ::= `const` ( IDENTIFIER | `_` ) `:` Type ( `=` Expression )? `;`

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

    fn make_key(&self, rule: &'static str) -> MemoKey {
        MemoKey {
            rule,
            position: self.tokens.get_position(),
        }
    }

    fn get_memo(&mut self, key: &MemoKey) -> Option<MemoResult> {
        let memo = self.get_memo_uncheck(key)?;
        if matches!(memo, MemoResult::Pending) {
            return None; // 左再帰を起こしている
        }

        Some(memo)
    }

    fn get_memo_uncheck(&mut self, key: &MemoKey) -> Option<MemoResult> {
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

    fn make_span(&self, key: &MemoKey) -> Span {
        let start = self
            .tokens
            .get_from_index(key.position)
            .map(|token| token.position)
            .unwrap_or(0);
        let end = if self.tokens.get_position() > key.position {
            self.tokens.token_end(self.tokens.get_position() - 1)
        } else {
            start
        };
        Span {
            file_id: 0, // 仮
            start: start as u32,
            end: end as u32,
        }
    }

    fn make_memo_from<N>(&mut self, key: MemoKey, node_kind: N) -> ParseResult
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

    fn make_memo_from_node(&mut self, key: MemoKey, node: Rc<ASTNode>) -> ParseResult {
        let memo = MemoResult::Match {
            memo: Memo::ASTNode(node),
            next_position: self.tokens.get_position(),
        };
        self.make_memo_from_result(key, Ok(memo))
    }

    fn make_memo_from_result(&mut self, key: MemoKey, result: ParseResult) -> ParseResult {
        let memo = match &result {
            Ok(memo) => {
                let pos = self.tokens.get_position();
                if self.max_read_position < pos {
                    self.max_read_position = pos;
                }

                memo.clone()
            }
            Err(error) => {
                let position = self.tokens.get_position();
                if position > self.max_read_position || self.last_fail_rule.is_none() {
                    self.max_read_position = position;
                    self.last_fail_rule = Some(key.rule);
                }
                if matches!(
                    error,
                    ParseError::IntErr(_)
                        | ParseError::FloatErr(_)
                        | ParseError::Unsupported { .. }
                ) {
                    let start = self
                        .tokens
                        .get_from_index(key.position)
                        .map(|token| token.position)
                        .unwrap_or(self.tokens.source_end());
                    let end = self.tokens.token_end(position.saturating_sub(1));
                    self.failure = Some((start, end, error.clone()));
                } else if matches!(error, ParseError::Expected { .. }) {
                    let start = self
                        .tokens
                        .get()
                        .map(|token| token.position)
                        .unwrap_or(self.tokens.source_end());
                    if self
                        .failure
                        .as_ref()
                        .map(|(previous, _, error)| {
                            matches!(error, ParseError::Expected { .. }) && start >= *previous
                        })
                        .unwrap_or(true)
                    {
                        self.failure =
                            Some((start, self.tokens.token_end(position), error.clone()));
                    }
                }
                self.tokens.set_position(key.position);
                MemoResult::NotMatch
            }
        };
        self.memo.insert(key, memo);

        result
    }
}
