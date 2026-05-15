use alloc::collections::btree_map::BTreeMap;
use alloc::vec::Vec;
use musubu_name_space::*;
use musubu_primitive::PrimitiveType;
use musubu_scope::*;
use musubu_type_check::errors::TypeCheckError;

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
        Ok(self
            .get_mut_name_space()
            .ok_or(TypeCheckError::InvalidScope)?
            .add_struct(struct_item)?)
    }

    fn add_function(&mut self, function_item: FunctionItem<'a>) -> Result<(), ResolveError> {
        Ok(self
            .get_mut_name_space()
            .ok_or(TypeCheckError::InvalidScope)?
            .add_function(function_item)?)
    }

    fn add_enumeration(&mut self, enum_item: EnumItem<'a>) -> Result<(), ResolveError> {
        Ok(self
            .get_mut_name_space()
            .ok_or(TypeCheckError::InvalidScope)?
            .add_enumeration(enum_item)?)
    }

    fn get_struct(&self, name: &'a str) -> Result<&StructItem<'a>, ResolveError> {
        Ok(self
            .get_name_space()
            .ok_or(TypeCheckError::InvalidScope)?
            .get_struct(name)?)
    }

    fn get_function(&self, name: &'a str) -> Result<&FunctionItem<'a>, ResolveError> {
        Ok(self
            .get_name_space()
            .ok_or(TypeCheckError::InvalidScope)?
            .get_function(name)?)
    }

    fn get_enumeration(&self, name: &'a str) -> Result<&EnumItem<'a>, ResolveError> {
        Ok(self
            .get_name_space()
            .ok_or(TypeCheckError::InvalidScope)?
            .get_enumeration(name)?)
    }
}

impl<'a> SymbolStore<'a, ResolveError> for NameResolver<'a> {
    fn get_type_option(&self, name: &'a str) -> Option<&TypeOption> {
        self.get_scope()?.get_type_option(name)
    }

    fn get_type(&self, name: &'a str) -> Option<&TypeSymbol> {
        self.get_scope()?.get_type(name)
    }

    fn resolve_variable_type(
        &mut self,
        name: &'a str,
        ty: PrimitiveType,
    ) -> Result<(), ResolveError> {
        Ok(self
            .get_mut_scope()
            .ok_or(TypeCheckError::InvalidScope)?
            .resolve_variable_type(name, ty)?)
    }

    fn add_variable(&mut self, name: &'a str, ty: TypeRequirement) -> Result<(), ResolveError> {
        Ok(self
            .get_mut_scope()
            .ok_or(TypeCheckError::InvalidScope)?
            .add_variable(name, ty)?)
    }

    fn add_type(&mut self, name: &'a str, ty: TypeRequirement) -> Result<(), ResolveError> {
        Ok(self
            .get_mut_scope()
            .ok_or(TypeCheckError::InvalidScope)?
            .add_type(name, ty)?)
    }

    fn contains(&self, name: &'a str) -> bool {
        self.get_scope().is_some_and(|scope| scope.contains(name))
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
        if let Some(module) = self.get_mut_name_space() {
            module.add_module(name);
        } else {
            self.modules.entry(name).or_insert(Module::new(name));
        }

        self.current_module_path.push(name);
        Ok(())
    }

    pub fn exit_module(&mut self) -> ResolveResult<()> {
        self.current_module_path
            .pop()
            .ok_or(ResolveError::InvalidScope)?;
        Ok(())
    }

    pub(crate) fn get_scope(&self) -> Option<&Scope<'a>> {
        self.scope_stack.last()
    }

    pub(crate) fn get_mut_scope(&mut self) -> Option<&mut Scope<'a>> {
        self.scope_stack.last_mut()
    }

    fn get_top_level_module(&self) -> Option<&Module<'a>> {
        // root(project)->toplevel(crate)->module
        let module_name = self.current_module_path.first()?;
        self.modules.get(module_name)
    }

    fn get_mut_top_level_module(&mut self) -> Option<&mut Module<'a>> {
        // root(project)->toplevel(crate)->module
        let module_name = self.current_module_path.first()?;
        self.modules.get_mut(module_name)
    }

    fn get_name_space(&self) -> Option<&Module<'a>> {
        let path = self.get_child_path();
        self.get_top_level_module()?
            .get_last_module(path.as_slice())
    }

    fn get_mut_name_space(&mut self) -> Option<&mut Module<'a>> {
        let path = self.get_child_path();
        self.get_mut_top_level_module()?
            .get_mut_last_module(path.as_slice())
    }

    fn get_child_path(&self) -> Vec<&'a str> {
        self.current_module_path
            .clone()
            .into_iter()
            .skip(1) // 最初の要素はトップレベルなので含めない
            .collect::<Vec<_>>()
    }
}
