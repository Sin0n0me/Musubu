use alloc::collections::btree_map::BTreeMap;
use alloc::vec::Vec;
use musubu_name_space::*;
use musubu_scope::*;

use crate::{ResolveResult, errors::ResolveError};

#[derive(Debug)]
pub struct NameResolver<'a> {
    modules: BTreeMap<&'a str, Module<'a>>, // 定義済みモジュール
    current_module: Option<&'a mut Module<'a>>, //
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
            current_module: None,
            current_module_path: Vec::new(),
            scope_stack: Vec::new(),
        }
    }

    pub fn enter_module(&mut self, name: &'a str) -> ResolveResult<()> {
        self.modules;

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
        self.current_module_path
            .last()
            .ok_or(ResolveError::InvalidModule)
    }

    fn get_mut_name_space(&mut self) -> ResolveResult<&mut Module<'a>> {
        self.current_module_path
            .last_mut()
            .ok_or(ResolveError::InvalidModule)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ResolvedPath<'a> {
    path: Vec<&'a str>,
}

impl<'a> ResolvedPath<'a> {
    pub fn new(name: &'a str) -> Self {
        Self { path: vec![name] }
    }

    pub fn add_child(&mut self, name: &'a str) {
        self.path.push(name);
    }

    pub fn parent_path(&self) -> Self {
        let mut path = self.clone();
        path.path.pop();
        path
    }

    pub fn last_name(&self) -> Option<&'a str> {
        self.path.last().map(|p| *p)
    }

    pub fn from(path: &'a Path) -> Self {
        let mut iter = path.segments.iter();
        let Some(root) = iter.next() else {
            return Self::new("");
        };
        let mut new_path = Self::new(&root.node.ident);
        for segment in iter {
            new_path.add_child(&segment.node.ident);
        }

        new_path
    }

    pub fn get_path(&self) -> &Vec<&'a str> {
        &self.path
    }
}
