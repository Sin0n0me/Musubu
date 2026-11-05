use crate::{ResolveResult, errors::ResolveError};
use alloc::{collections::BTreeSet, vec::Vec};
use musubu_hir::{
    HIRBlock, HIRExpression as Expr, HIRFunctionParam, HIRMatchArm, HIRMatchPattern, HIRStatement,
};
use musubu_span::Span;

// None denotes an unreachable path, not an empty set of initialized variables.
type State = Option<BTreeSet<usize>>;

pub(super) fn check(body: &HIRBlock, parameters: &[HIRFunctionParam]) -> ResolveResult<()> {
    let mut checker = Checker { breaks: Vec::new() };
    let mut state = Some(parameters.iter().map(|p| p.argument).collect());
    checker.block(body, &mut state)
}

fn merge(left: State, right: State) -> State {
    match (left, right) {
        (Some(mut left), Some(right)) => {
            left.retain(|id| right.contains(id));
            Some(left)
        }
        (left, None) => left,
        (None, right) => right,
    }
}

struct Checker {
    breaks: Vec<State>,
}

impl Checker {
    fn block(&mut self, block: &HIRBlock, state: &mut State) -> ResolveResult<()> {
        for statement in &block.statements {
            match statement {
                HIRStatement::Let {
                    symbol,
                    initializer,
                    ..
                } => {
                    // A declaration inside a loop starts a fresh binding on every iteration.
                    if let Some(initialized) = state {
                        initialized.remove(symbol);
                    }
                    if let Some(value) = initializer {
                        self.expression(value, state)?;
                        Self::initialize(*symbol, state);
                    }
                }
                HIRStatement::Expr(value) | HIRStatement::Discard(value) => {
                    self.expression(value, state)?
                }
            }
        }
        Ok(())
    }

    fn initialize(id: usize, state: &mut State) {
        if let Some(initialized) = state {
            initialized.insert(id);
        }
    }

    fn read(id: usize, state: &State) -> ResolveResult<()> {
        if state
            .as_ref()
            .is_some_and(|initialized| !initialized.contains(&id))
        {
            Err(ResolveError::UninitializedVariable)
        } else {
            Ok(())
        }
    }

    fn expression(&mut self, expr: &Expr, state: &mut State) -> ResolveResult<()> {
        if state.is_none() {
            return Ok(());
        }
        match expr {
            Expr::Located { range, expression } => {
                self.expression(expression, state).map_err(|e| {
                    e.at(Span {
                        start: range.0 as u32,
                        end: range.1 as u32,
                        file_id: 0,
                    })
                })
            }
            Expr::Variable { id, .. } => Self::read(*id, state),
            Expr::Literal(_) => Ok(()),
            Expr::Block(body) => self.block(body, state),
            Expr::Store { target, value } => {
                self.expression(value, state)?;
                Self::initialize(*target, state);
                Ok(())
            }
            Expr::StoreField { target, value, .. } => {
                self.expression(value, state)?;
                Self::read(*target, state)
            }
            Expr::StoreIndex {
                target,
                path,
                value,
                ..
            } => self.store_index(*target, path, value, state),
            Expr::Field { parent, .. } => self.expression(parent, state),
            Expr::Index {
                parent: lhs,
                index: rhs,
                ..
            }
            | Expr::BinOp { lhs, rhs, .. }
            | Expr::CmpOp { lhs, rhs, .. }
            | Expr::Range {
                start: lhs,
                end: rhs,
                ..
            } => {
                self.expression(lhs, state)?;
                self.expression(rhs, state)
            }
            Expr::ArrayRepeat { value, .. } => self.expression(value, state),
            Expr::Array { elements, .. }
            | Expr::Call {
                arguments: elements,
                ..
            } => self.sequence(elements.iter(), state),
            Expr::Struct { fields, .. } | Expr::Enum { fields, .. } => {
                self.sequence(fields.iter().map(|(_, value)| value), state)
            }
            Expr::If {
                cond,
                then_block,
                else_block,
            } => self.conditional(cond, then_block, else_block.as_ref(), state),
            Expr::Match { value, arms, .. } => self.match_arms(value, arms, state),
            Expr::Loop { body, .. } => self.loop_body(body, state),
            Expr::For {
                symbol,
                iterator,
                body,
            } => self.for_body(*symbol, iterator, body, state),
            Expr::Return(value) => {
                self.exit(value.as_deref(), state)?;
                Ok(())
            }
            Expr::Break(value) => {
                let exit = self.exit(value.as_deref(), state)?;
                let exits = self.breaks.last_mut().ok_or(ResolveError::IllegalBreak)?;
                *exits = merge(exits.take(), exit);
                Ok(())
            }
            Expr::Continue => {
                if self.breaks.is_empty() {
                    return Err(ResolveError::IllegalContinue);
                }
                *state = None;
                Ok(())
            }
        }
    }

    fn sequence<'a>(
        &mut self,
        values: impl Iterator<Item = &'a Expr>,
        state: &mut State,
    ) -> ResolveResult<()> {
        for value in values {
            self.expression(value, state)?;
        }
        Ok(())
    }

    fn store_index(
        &mut self,
        target: usize,
        path: &[Expr],
        value: &Expr,
        state: &mut State,
    ) -> ResolveResult<()> {
        self.sequence(path.iter(), state)?;
        // Indexed stores load the existing parent before evaluating the RHS.
        Self::read(target, state)?;
        self.expression(value, state)
    }

    fn conditional(
        &mut self,
        cond: &Expr,
        then_body: &HIRBlock,
        else_body: Option<&HIRBlock>,
        state: &mut State,
    ) -> ResolveResult<()> {
        self.expression(cond, state)?;
        let mut other = state.clone();
        self.block(then_body, state)?;
        if let Some(body) = else_body {
            self.block(body, &mut other)?;
        }
        *state = merge(state.take(), other);
        Ok(())
    }

    fn match_arms(
        &mut self,
        value: &Expr,
        arms: &[HIRMatchArm],
        state: &mut State,
    ) -> ResolveResult<()> {
        self.expression(value, state)?;
        let mut joined = None;
        for arm in arms {
            let mut branch = state.clone();
            Self::pattern(&arm.pattern, &mut branch);
            self.expression(&arm.body, &mut branch)?;
            joined = merge(joined, branch);
        }
        *state = joined;
        Ok(())
    }

    fn loop_body(&mut self, body: &HIRBlock, state: &mut State) -> ResolveResult<()> {
        self.breaks.push(None);
        let result = self.block(body, state);
        // Restore the loop context on both success and diagnostic propagation.
        let exits = self.breaks.pop().flatten();
        result?;
        // Only breaks reach the statement after an unconditional loop.
        *state = exits;
        Ok(())
    }

    fn for_body(
        &mut self,
        symbol: usize,
        iterator: &Expr,
        body: &HIRBlock,
        state: &mut State,
    ) -> ResolveResult<()> {
        self.expression(iterator, state)?;
        let mut iteration = state.clone();
        self.breaks.push(None);
        Self::initialize(symbol, &mut iteration);
        let result = self.block(body, &mut iteration);
        self.breaks.pop();
        // The iterable can be empty, so body assignments cannot establish initialization.
        result
    }

    fn exit(&mut self, value: Option<&Expr>, state: &mut State) -> ResolveResult<State> {
        if let Some(value) = value {
            self.expression(value, state)?;
        }
        Ok(state.take())
    }

    fn pattern(pattern: &HIRMatchPattern, state: &mut State) {
        match pattern {
            HIRMatchPattern::Binding(id) => Self::initialize(*id, state),
            HIRMatchPattern::Tuple(fields) | HIRMatchPattern::Variant { fields, .. } => {
                for (_, pattern) in fields {
                    Self::pattern(pattern, state);
                }
            }
            HIRMatchPattern::Wildcard => (),
        }
    }
}
