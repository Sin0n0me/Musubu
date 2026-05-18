// TODO
//#![no_std]

extern crate alloc;

pub mod errors;

mod name_resolver;
mod resolver;
mod resolver_collect;

use crate::resolver_collect::SymbolCollector;
use errors::ResolveError;
use musubu_scope::errors::ScopeError;
use musubu_scope::{Scope, ScopeControl, TypeSymbol};
use musubu_type_check::TypeChecker;
use name_resolver::NameResolver;

pub type ResolveResult<T> = Result<T, ResolveError>;

#[derive(Debug)]
pub struct Resolver<'a> {
    pub name_resolver: NameResolver<'a>,
    type_checker: TypeChecker,
    collector: SymbolCollector<'a>,
}

impl<'a> Resolver<'a> {
    pub fn new(project_name: &'a str) -> Self {
        Self {
            name_resolver: NameResolver::new(project_name),
            type_checker: TypeChecker::new(),
            collector: SymbolCollector::new(),
        }
    }

    pub(crate) fn enter_function(
        &mut self,
        return_type: TypeSymbol,
        function: impl Fn(&mut Self) -> ResolveResult<TypeSymbol>,
    ) -> ResolveResult<()> {
        self.type_checker.enter_function(return_type);

        let result = self.enter_scope(function)?;

        self.type_checker.check_return(result)?;
        self.type_checker.exit_function();

        Ok(())
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

    pub(crate) fn enter_scope(
        &mut self,
        function: impl Fn(&mut Self) -> ResolveResult<TypeSymbol>,
    ) -> ResolveResult<TypeSymbol> {
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
