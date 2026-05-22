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
use musubu_hir::HIRModule;
use musubu_scope::{Scope, ScopeControl, TypeSymbol, errors::ScopeError};
use musubu_span::SpannedAsRef;
use musubu_type_check::TypeChecker;

pub type ResolveResult<T> = Result<T, ResolveError>;

// TODO 分割したコードをまとめて取り込めるように

// 逐次解決
pub fn resolve_sequential(
    project_name: &str,
    module_name: &str,
    ast_items: &[&ASTNode],
) -> ResolveResult<HIRModule> {
    let mut hir = HIRModule::new();
    let mut resolver = Resolver::new(project_name, &mut hir);

    resolver.resolve(module_name, ast_items)?;

    Ok(hir)
}

// 完全な解決
// 推論を強力にするならresolve内のdesuger部分の分離が必要
pub fn resolve_unordered(
    project_name: &str,
    module_name: &str,
    ast_items: &[&ASTNode],
) -> ResolveResult<HIRModule> {
    let mut hir = HIRModule::new();
    let mut resolver = Resolver::new(project_name, &mut hir);

    resolver.import(module_name, ast_items)?;
    resolver.resolve(module_name, ast_items)?;

    Ok(hir)
}

#[derive(Debug)]
struct Resolver<'a> {
    name_resolver: NameResolver<'a>,
    hir_module: &'a mut HIRModule,
    type_checker: TypeChecker,
    collector: SymbolCollector<'a>,
    desuger: Desugar<'a>,
}

#[derive(Debug)]
struct Lowered<T> {
    type_symbol: TypeSymbol,
    hir: T,
}

impl<T> Lowered<T> {
    fn split(self) -> (T, TypeSymbol) {
        (self.hir, self.type_symbol)
    }
}

impl<'a> Resolver<'a> {
    pub fn new(project_name: &'a str, hir_module: &'a mut HIRModule) -> Self {
        Self {
            name_resolver: NameResolver::new(project_name),
            hir_module,
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

        let mut result = self.enter_scope(function)?;

        result.type_symbol = self.type_checker.check_return(Some(result.type_symbol))?;
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
