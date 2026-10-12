use super::*;

fn inferred() -> TypeRequirement {
    TypeRequirement::Inferring(TypeOption {
        mutable: true,
        reference: true,
    })
}

#[test]
fn missing_symbols_have_no_id_type_or_options() {
    let scope = Scope::new();
    assert!(!scope.contains("missing"));
    assert!(!scope.is_variable("missing"));
    assert!(!scope.is_type("missing"));
    assert!(scope.get_variable_id("missing").is_none());
    assert!(scope.get_type("missing").is_none());
    assert!(scope.get_type_option("missing").is_none());
}

#[test]
fn resolving_inference_preserves_binding_options() {
    let mut scope = Scope::new();
    scope.add_variable(42, "value", inferred()).unwrap();
    assert!(scope.contains_inferring_types());
    assert!(scope.get_type("value").is_none());
    scope
        .resolve_variable_type("value", PrimitiveType::default_integer())
        .unwrap();
    assert_eq!(scope.get_variable_id("value"), Some(&42));
    let ty = scope.get_type("value").unwrap();
    assert_eq!(ty.type_kind, PrimitiveType::default_integer());
    assert!(ty.is_mutable());
    assert!(ty.is_reference());
    assert!(!scope.contains_inferring_types());
    assert!(scope.get_inferring_names().is_empty());
}

#[test]
fn resolving_twice_preserves_original_type() {
    let mut scope = Scope::new();
    scope.add_variable(1, "x", inferred()).unwrap();
    scope
        .resolve_variable_type("x", PrimitiveType::default_integer())
        .unwrap();
    assert!(matches!(
        scope.resolve_variable_type("x", PrimitiveType::default_float()),
        Err(ScopeError::TypeConflict { .. })
    ));
    assert_eq!(
        scope.get_type("x").unwrap().type_kind,
        PrimitiveType::default_integer()
    );
}

#[test]
fn resolving_missing_variable_reports_its_name() {
    let mut scope = Scope::new();
    assert!(
        matches!(scope.resolve_variable_type("missing", PrimitiveType::Unit), Err(ScopeError::UnresolvedVariable { name }) if name == "missing")
    );
}

#[test]
fn type_symbol_cannot_be_resolved_as_variable() {
    let mut scope = Scope::new();
    scope.add_type("T", inferred()).unwrap();
    assert!(scope.is_type("T"));
    assert!(!scope.is_variable("T"));
    assert!(scope.get_variable_id("T").is_none());
    assert!(matches!(
        scope.resolve_variable_type("T", PrimitiveType::Unit),
        Err(ScopeError::NotVariable { .. })
    ));
}

#[test]
fn duplicate_variable_keeps_original_id_and_type() {
    let mut scope = Scope::new();
    scope.add_variable(7, "x", inferred()).unwrap();
    assert!(matches!(
        scope.add_variable(9, "x", TypeRequirement::Expect(TypeSymbol::default())),
        Err(ScopeError::DuplicateVariable { .. })
    ));
    assert_eq!(scope.get_variable_id("x"), Some(&7));
    assert!(scope.contains_inferring_types());
}

#[test]
fn types_and_variables_share_a_namespace() {
    let mut scope = Scope::new();
    scope
        .add_type(
            "T",
            TypeRequirement::Expect(TypeSymbol::new(PrimitiveType::default_float())),
        )
        .unwrap();
    assert!(matches!(
        scope.add_variable(1, "T", inferred()),
        Err(ScopeError::DuplicateType { .. })
    ));
    scope.add_variable(1, "x", inferred()).unwrap();
    assert!(matches!(
        scope.add_type("x", inferred()),
        Err(ScopeError::DuplicateVariable { .. })
    ));
    assert_eq!(
        scope.get_type("T").unwrap().type_kind,
        PrimitiveType::default_float()
    );
}

#[test]
fn inference_names_exclude_resolved_bindings() {
    let mut scope = Scope::new();
    scope.add_variable(1, "a", inferred()).unwrap();
    scope.add_variable(2, "b", inferred()).unwrap();
    scope.add_type("T", inferred()).unwrap();
    scope
        .resolve_variable_type("a", PrimitiveType::Unit)
        .unwrap();
    let names = scope.get_inferring_names();
    assert_eq!(names.len(), 2);
    assert!(names.iter().any(|name| name == "b"));
    assert!(names.iter().any(|name| name == "T"));
}

#[test]
fn type_identity_ignores_mutability_but_not_reference() {
    let plain = TypeSymbol::new(PrimitiveType::default_integer());
    let mut other = plain.clone();
    other.option.mutable = true;
    assert!(plain.is_same_type(&other));
    other.option.reference = true;
    assert!(!plain.is_same_type(&other));
    assert!(!plain.is_same_type(&TypeSymbol::new(PrimitiveType::default_float())));
}
