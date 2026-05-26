use crate::{ResolveResult, errors::ResolveError};
use alloc::vec::Vec;
use core::iter::once;
use musubu_name_space::*;
use musubu_primitive::PrimitiveType;
use musubu_scope::errors::ScopeError;
use musubu_scope::*;

#[derive(Debug)]
pub(crate) struct NameResolver<'a> {
    root_module: Module<'a>,           // 定義済みモジュール
    current_module_path: Vec<&'a str>, // 現在 stack
    scope_stack: Vec<Scope<'a>>,       // 一時
}

impl<'a> ScopeControl<ResolveError> for NameResolver<'a> {
    fn on_enter_scope(&mut self) -> Result<(), ResolveError> {
        self.scope_stack.push(Scope::new());
        Ok(())
    }

    fn on_exit_scope(&mut self) -> Result<(), ResolveError> {
        let scope = self
            .scope_stack
            .pop()
            .ok_or(ResolveError::ScopeError(ScopeError::InvalidScope))?;

        let inffering_names = scope.get_inferring_names();
        if !inffering_names.is_empty() {
            return Err(ResolveError::UnresolveTypes {
                names: inffering_names,
            });
        }

        Ok(())
    }
}

impl<'a> ItemStore<'a, ResolveError> for NameResolver<'a> {
    fn add_struct(&mut self, struct_item: StructItem<'a>) -> Result<(), ResolveError> {
        Ok(self
            .get_mut_name_space()
            .ok_or(ScopeError::InvalidScope)?
            .add_struct(struct_item)?)
    }

    fn add_function(&mut self, function_item: FunctionItem<'a>) -> Result<(), ResolveError> {
        Ok(self
            .get_mut_name_space()
            .ok_or(ScopeError::InvalidScope)?
            .add_function(function_item)?)
    }

    fn add_enumeration(&mut self, enum_item: EnumItem<'a>) -> Result<(), ResolveError> {
        Ok(self
            .get_mut_name_space()
            .ok_or(ScopeError::InvalidScope)?
            .add_enumeration(enum_item)?)
    }
}

impl<'a> ItemStoreReader<'a> for NameResolver<'a> {
    fn get_struct(&self, name: &'a str) -> Option<&StructItem<'a>> {
        self.get_name_space()?.get_struct(name)
    }

    fn get_function(&self, name: &'a str) -> Option<&FunctionItem<'a>> {
        self.get_name_space()?.get_function(name)
    }

    fn get_enumeration(&self, name: &'a str) -> Option<&EnumItem<'a>> {
        self.get_name_space()?.get_enumeration(name)
    }
}

impl<'a> SymbolStore<'a, ResolveError> for NameResolver<'a> {
    fn get_type(&self, name: &'a str) -> Option<&TypeSymbol> {
        self.get_scope()?.get_type(name)
    }

    fn get_type_option(&self, name: &'a str) -> Option<&TypeOption> {
        self.get_scope()?.get_type_option(name)
    }

    fn is_variable(&self, name: &'a str) -> bool {
        self.get_scope().is_some_and(|s| s.is_variable(name))
    }

    fn is_type(&self, name: &'a str) -> bool {
        self.get_scope().is_some_and(|s| s.is_type(name))
    }

    fn resolve_variable_type(
        &mut self,
        name: &'a str,
        ty: PrimitiveType,
    ) -> Result<(), ResolveError> {
        Ok(self
            .get_mut_scope()
            .ok_or(ScopeError::InvalidScope)?
            .resolve_variable_type(name, ty)?)
    }

    fn add_variable(&mut self, name: &'a str, ty: TypeRequirement) -> Result<(), ResolveError> {
        Ok(self
            .get_mut_scope()
            .ok_or(ScopeError::InvalidScope)?
            .add_variable(name, ty)?)
    }

    fn add_type(&mut self, name: &'a str, ty: TypeRequirement) -> Result<(), ResolveError> {
        Ok(self
            .get_mut_scope()
            .ok_or(ScopeError::InvalidScope)?
            .add_type(name, ty)?)
    }

    fn contains(&self, name: &'a str) -> bool {
        self.get_scope().is_some_and(|scope| scope.contains(name))
    }
}

impl<'a> NameResolver<'a> {
    pub fn new(project_name: &'a str) -> Self {
        Self {
            root_module: Module::new(project_name),
            current_module_path: Vec::new(),
            scope_stack: Vec::new(),
        }
    }

    pub fn enter_module(&mut self, name: &'a str) -> ResolveResult<()> {
        self.current_module_path.push(name);
        self.root_module
            .add_modules(self.current_module_path.as_slice());

        Ok(())
    }

    pub fn exit_module(&mut self) -> ResolveResult<()> {
        self.current_module_path
            .pop()
            .ok_or(ResolveError::InvalidModuleScope)?;

        Ok(())
    }

    pub fn get_type(&self, name: &'a str) -> Option<&TypeSymbol> {
        // 変数や型が優先される
        if let Some(symbol) = self.get_scope()?.get_type(name) {
            return Some(symbol);
        }
        if let Some(symbol) = self.get_name_space()?.get_type(name) {
            return Some(symbol);
        }

        None
    }

    pub fn get_item(&self, name: &'a str) -> Option<&ItemSymbol<'a>> {
        self.get_name_space()?.get_item(name)
    }

    pub fn get_scope(&self) -> Option<&Scope<'a>> {
        self.scope_stack.last()
    }

    pub fn get_mut_scope(&mut self) -> Option<&mut Scope<'a>> {
        self.scope_stack.last_mut()
    }

    pub fn get_full_path(&self, name: &'a str) -> Vec<&'a str> {
        self.current_module_path
            .iter()
            .copied()
            .chain(once(name))
            .collect()
    }

    fn get_top_level_module(&self) -> Option<&Module<'a>> {
        // root(project)->toplevel(crate)->module
        let module_name = self.current_module_path.first()?;
        self.root_module.get_module(module_name)
    }

    fn get_mut_top_level_module(&mut self) -> Option<&mut Module<'a>> {
        // root(project)->toplevel(crate)->module
        let module_name = self.current_module_path.first()?;
        self.root_module.get_mut_module(module_name)
    }

    fn get_name_space(&self) -> Option<&Module<'a>> {
        let path = self.get_child_path();
        self.get_top_level_module()?
            .get_module_from_path(path.as_slice())
    }

    fn get_mut_name_space(&mut self) -> Option<&mut Module<'a>> {
        let path = self.get_child_path();
        self.get_mut_top_level_module()?
            .get_mut_module_from_path(path.as_slice())
    }

    fn get_child_path(&self) -> Vec<&'a str> {
        self.current_module_path
            .clone()
            .into_iter()
            .skip(1) // 最初の要素はトップレベルなので含めない
            .collect::<Vec<_>>()
    }
}
