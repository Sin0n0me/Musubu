use super::*;

#[test]
fn adding_nested_modules_preserves_siblings_and_existing_items() {
    let mut root = Module::new("root");
    root.add_modules(&["a", "b"]);
    root.get_mut_module_from_path(&["a", "b"])
        .unwrap()
        .add_function(FunctionItem::new(7, "f", TypeSymbol::default()))
        .unwrap();
    root.add_modules(&["a", "c"]);
    root.add_modules(&["a", "b"]);
    root.add_module("a");
    assert_eq!(
        root.get_module_from_path(&["a", "b"])
            .unwrap()
            .get_function("f")
            .unwrap()
            .id,
        7
    );
    assert!(root.get_module_from_path(&["a", "c"]).is_some());
    assert!(root.get_module("missing").is_none());
}

#[test]
fn empty_module_path_refers_to_root() {
    let mut root = Module::new("root");
    root.add_modules(&[]);
    root.get_mut_module_from_path(&[])
        .unwrap()
        .add_struct(StructItem::new("S"))
        .unwrap();
    assert!(
        root.get_module_from_path(&[])
            .unwrap()
            .get_struct("S")
            .is_some()
    );
}

#[test]
fn item_lookup_distinguishes_function_struct_and_enum() {
    let mut root = Module::new("root");
    root.add_function(FunctionItem::new(1, "f", TypeSymbol::default()))
        .unwrap();
    root.add_struct(StructItem::new("S")).unwrap();
    root.add_enumeration(EnumItem::new("E")).unwrap();
    assert!(root.get_function("S").is_none());
    assert!(root.get_struct("E").is_none());
    assert!(root.get_enumeration("f").is_none());
    assert!(root.get_type("missing").is_none());
    assert!(root.get_item("missing").is_none());
    assert!(matches!(
        root.get_type("f").unwrap().type_kind,
        PrimitiveType::Function { .. }
    ));
}

#[test]
fn struct_field_order_is_declaration_order_not_alphabetic() {
    let mut item = StructItem::new("S");
    item.add_field("z", TypeSymbol::new(PrimitiveType::default_integer()))
        .unwrap();
    item.add_field("a", TypeSymbol::new(PrimitiveType::Boolean))
        .unwrap();
    let PrimitiveType::NamedStruct { name, fields } = item.to_type() else {
        panic!("expected struct")
    };
    assert_eq!(name, "S");
    assert_eq!(
        fields,
        alloc::vec![
            ("z".into(), PrimitiveType::default_integer()),
            ("a".into(), PrimitiveType::Boolean)
        ]
    );
}

#[test]
fn function_type_preserves_argument_order_and_return_type() {
    let mut item = FunctionItem::new(9, "f", TypeSymbol::new(PrimitiveType::Boolean));
    item.add_argument(TypeSymbol::new(PrimitiveType::default_integer()))
        .unwrap();
    item.add_argument(TypeSymbol::new(PrimitiveType::default_float()))
        .unwrap();
    assert_eq!(
        item.to_type(),
        PrimitiveType::Function {
            return_type: Box::new(PrimitiveType::Boolean),
            arguments: alloc::vec![
                PrimitiveType::default_integer(),
                PrimitiveType::default_float()
            ],
        }
    );
}

#[test]
fn enum_variant_order_and_shape_are_preserved() {
    let mut item = EnumItem::new("E");
    item.add_variant("Z").unwrap();
    item.add_variant("A").unwrap();
    item.variant_kinds.insert("A", EnumVariantKind::Tuple);
    item.add_variant_field("A", "0", TypeSymbol::new(PrimitiveType::Boolean))
        .unwrap();
    let PrimitiveType::NamedEnum { name, variants } = item.to_type() else {
        panic!("expected enum")
    };
    assert_eq!(name, "E");
    assert_eq!(variants.len(), 2);
    assert_eq!(variants[0].name, "Z");
    assert_eq!(variants[0].kind, EnumVariantKind::Unit);
    assert!(variants[0].fields.is_empty());
    assert_eq!(variants[1].name, "A");
    assert_eq!(variants[1].kind, EnumVariantKind::Tuple);
    assert_eq!(
        variants[1].fields,
        alloc::vec![("0".into(), PrimitiveType::Boolean)]
    );
}

#[test]
fn field_of_missing_enum_variant_is_rejected() {
    let mut item = EnumItem::new("E");
    assert!(
        matches!(item.add_variant_field("Missing", "x", TypeSymbol::default()), Err(NameSpaceError::UnresolvedEnumVariant { name }) if name == "Missing")
    );
    assert!(item.variants.is_empty());
}
