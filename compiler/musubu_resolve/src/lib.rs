// TODO
//#![no_std]

extern crate alloc;

pub mod errors;
pub mod name_resolver;

mod resolver;
mod resolver_collect;

use crate::name_resolver::NameResolver;
use crate::resolver_collect::SymbolCollector;
use errors::ResolveError;
use musubu_ast::ASTNode;
use musubu_desugar::Desugar;
use musubu_hir::{HIRExpression, HIRFunction, HIRStatement};
use musubu_scope::{Scope, ScopeControl, TypeSymbol, errors::ScopeError};
use musubu_span::SpannedAsRef;
use musubu_type_check::TypeChecker;

pub type ResolveResult<T> = Result<T, ResolveError>;

#[derive(Debug)]
pub struct Resolver<'a> {
    pub name_resolver: NameResolver<'a>,
    type_checker: TypeChecker,
    collector: SymbolCollector<'a>,
    desuger: Desugar<'a>,
}

#[derive(Debug)]
pub struct Lowered<T> {
    type_symbol: TypeSymbol,
    hir: T,
}

impl<'a> Resolver<'a> {
    pub fn new(project_name: &'a str) -> Self {
        Self {
            name_resolver: NameResolver::new(project_name),
            type_checker: TypeChecker::new(),
            collector: SymbolCollector::new(),
            desuger: Desugar::new(),
        }
    }

    pub fn resolve(&mut self, module_name: &'a str, nodes: &[&'a ASTNode]) -> ResolveResult<()> {
        self.enter_module(module_name, |s| {
            for node in nodes {
                match node {
                    ASTNode::Item {
                        visibility: _,
                        item,
                    } => {
                        s.resolve_item(item.as_ref_spanned())?;
                    }
                    _ => unreachable!(),
                }
            }

            Ok(())
        })
    }

    pub(crate) fn enter_function<T>(
        &mut self,
        return_type: TypeSymbol,
        function: impl Fn(&mut Self) -> ResolveResult<Lowered<T>>,
    ) -> ResolveResult<Lowered<T>> {
        self.type_checker.enter_function(return_type);

        let result = self.enter_scope(function)?;

        self.type_checker.check_return(result.type_symbol)?;
        self.type_checker.exit_function();

        Ok(result)
    }

    pub(crate) fn enter_module(
        &mut self,
        module_name: &'a str,
        function: impl Fn(&mut Self) -> ResolveResult<()>,
    ) -> ResolveResult<()> {
        self.name_resolver.enter_module(module_name)?;
        self.name_resolver.on_enter_scope()?;
        self.type_checker.on_enter_scope()?;

        function(self)?;

        self.type_checker.on_exit_scope()?;
        self.name_resolver.on_exit_scope()?;
        self.name_resolver.exit_module()?;

        Ok(())
    }

    pub(crate) fn enter_scope<T>(
        &mut self,
        function: impl Fn(&mut Self) -> ResolveResult<Lowered<T>>,
    ) -> ResolveResult<Lowered<T>> {
        self.name_resolver.on_enter_scope()?;
        self.type_checker.on_enter_scope()?;

        let result = function(self)?;

        self.type_checker.on_exit_scope()?;
        self.name_resolver.on_exit_scope()?;

        Ok(result)
    }

    pub(crate) fn get_scope(&self) -> ResolveResult<&Scope<'a>> {
        self.name_resolver
            .get_scope()
            .ok_or(ResolveError::ScopeError(ScopeError::InvalidScope))
    }

    pub(crate) fn get_mut_scope(&mut self) -> ResolveResult<&mut Scope<'a>> {
        self.name_resolver
            .get_mut_scope()
            .ok_or(ResolveError::ScopeError(ScopeError::InvalidScope))
    }
}
