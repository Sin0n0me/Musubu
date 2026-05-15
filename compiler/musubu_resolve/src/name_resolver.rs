use alloc::collections::btree_map::BTreeMap;
use alloc::vec::Vec;
use musubu_name_space::*;
use musubu_scope::*;

use crate::{ResolveResult, errors::ResolveError};

#[derive(Debug)]
pub struct NameResolver<'a> {
    modules: BTreeMap<&'a str, Module<'a>>, // 定義済みモジュール
    current_module_path: Vec<&'a str>,      // 現在 stack
    scope_stack: Vec<Scope<'a>>,            // 一時
}

impl<'a> ScopeControl<ResolveError> for NameResolver<'a> {
    fn on_enter_scope(&mut self) -> Result<(), ResolveError> {
        self.scope_stack.push(Scope::new());
        Ok(())
    }

    fn on_exit_scope(&mut self) -> Result<(), ResolveError> {
        let scope = self.scope_stack.pop().ok_or(ResolveError::InvalidScope)?;

        Ok(())
    }
}

impl<'a> ItemStore<'a, ResolveError> for NameResolver<'a> {
    fn add_struct(&mut self, struct_item: StructItem<'a>) -> Result<(), ResolveError> {
        Ok(self.get_mut_name_space()?.add_struct(struct_item)?)
    }

    fn add_function(&mut self, function_item: FunctionItem<'a>) -> Result<(), ResolveError> {
        Ok(self.get_mut_name_space()?.add_function(function_item)?)
    }

    fn add_enumeration(&mut self, enum_item: EnumItem<'a>) -> Result<(), ResolveError> {
        Ok(self.get_mut_name_space()?.add_enumeration(enum_item)?)
    }

    fn get_struct(&self, name: &'a str) -> Result<&StructItem<'a>, ResolveError> {
        Ok(self.get_name_space()?.get_struct(name)?)
    }

    fn get_function(&self, name: &'a str) -> Result<&FunctionItem<'a>, ResolveError> {
        Ok(self.get_name_space()?.get_function(name)?)
    }

    fn get_enumeration(&self, name: &'a str) -> Result<&EnumItem<'a>, ResolveError> {
        Ok(self.get_name_space()?.get_enumeration(name)?)
    }
}

impl<'a> SymbolStore<'a, ResolveError> for NameResolver<'a> {
    fn get_type_option(&self, name: &'a str) -> Result<&TypeOption, ResolveError> {
        Ok(self.get_scope()?.get_type_option(name)?)
    }

    fn get_type(&self, name: &'a str) -> Result<&TypeSymbol, ResolveError> {
        Ok(self.get_scope()?.get_type(name)?)
    }

    fn add_variable(&mut self, name: &'a str, ty: TypeRequirement) -> Result<(), ResolveError> {
        Ok(self.get_mut_scope()?.add_variable(name, ty)?)
    }

    fn add_type(&mut self, name: &'a str, ty: TypeRequirement) -> Result<(), ResolveError> {
        Ok(self.get_mut_scope()?.add_type(name, ty)?)
    }

    fn contains(&self, name: &'a str) -> bool {
        self.get_scope().is_ok_and(|scope| scope.contains(name))
    }
}

impl<'a> NameResolver<'a> {
    pub fn new(project_name: &'a str) -> Self {
        Self {
            modules: BTreeMap::new(),
            current_module_path: Vec::new(),
            scope_stack: Vec::new(),
        }
    }

    pub fn enter_module(&mut self, name: &'a str) -> ResolveResult<()> {
        self.get_mut_name_space()?.add_module(name);
        self.current_module_path.push(name);
        Ok(())
    }

    pub fn exit_module(&mut self) -> ResolveResult<()> {
        self.current_module_path
            .pop()
            .ok_or(ResolveError::InvalidScope)?;
        Ok(())
    }

    fn get_scope(&self) -> ResolveResult<&Scope<'a>> {
        self.scope_stack.last().ok_or(ResolveError::InvalidScope)
    }

    fn get_mut_scope(&mut self) -> ResolveResult<&mut Scope<'a>> {
        self.scope_stack
            .last_mut()
            .ok_or(ResolveError::InvalidScope)
    }

    fn get_top_level_module(&self) -> ResolveResult<&Module<'a>> {
        let module_name = self
            .current_module_path
            .first()
            .ok_or(ResolveError::InvalidModule)?;
        self.modules
            .get(module_name)
            .ok_or(ResolveError::InvalidModule)
    }

    fn get_mut_top_level_module(&mut self) -> ResolveResult<&mut Module<'a>> {
        let module_name = self
            .current_module_path
            .first()
            .ok_or(ResolveError::InvalidModule)?;
        self.modules
            .get_mut(module_name)
            .ok_or(ResolveError::InvalidModule)
    }

    fn get_name_space(&self) -> ResolveResult<&Module<'a>> {
        let path = self.get_child_path();
        self.get_top_level_module()?
            .get_last_module(path.as_slice())
            .ok_or(ResolveError::InvalidModule)
    }

    fn get_mut_name_space(&mut self) -> ResolveResult<&mut Module<'a>> {
        let path = self.get_child_path();
        self.get_mut_top_level_module()?
            .get_mut_last_module(path.as_slice())
            .ok_or(ResolveError::InvalidModule)
    }

    fn get_child_path(&self) -> Vec<&'a str> {
        self.current_module_path
            .clone()
            .into_iter()
            .skip(1) // 最初の要素はトップレベルなので含めない
            .collect::<Vec<_>>()
    }
}
