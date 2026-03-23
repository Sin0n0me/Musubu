use musubu_primitive::*;
use musubu_span::*;

pub trait NodeMaker {
    fn make_node(self, span: Span) -> ASTNode;
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ASTNode {
    Item {
        visibility: Visibility,
        item: Spanned<Item>,
    },
    Expression(SpannedBox<Expression>),
    Type(Spanned<TypeKind>),
    TypeAlias(Spanned<TypeAlias>),
    Loop(Spanned<LoopExpr>),
    Visibility(Spanned<Visibility>),
    Statement(Spanned<Statement>),
    Statements(SpannedVec<Statement>),
    Pattern(Spanned<Pattern>),
    StructField(Spanned<StructField>),
    StructFields(SpannedVec<StructField>),
    EnumItem(Spanned<EnumItem>),
    EnumItems(SpannedVec<EnumItem>),
    CallParam(SpannedBox<Expression>),
    CallParams(Vec<SpannedBox<Expression>>),
    FunctionParameter(Spanned<FunctionParam>),
    FunctionParameters(SpannedVec<FunctionParam>),
    PathSegment(Spanned<PathSegment>),
    Field(Spanned<String>),
    Path(Spanned<Path>),
    Segment(Spanned<String>),
    Arguments(SpannedVec<TypeKind>),
}

impl ASTNode {}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Visibility {
    Private,
    Path(Path),
    Public,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Item {
    Function {
        name: String,
        params: SpannedVec<FunctionParam>,
        return_type: Option<Spanned<TypeKind>>,
        body: Option<SpannedBox<Expression>>,
    },
    Struct {
        name: String,
        fields: SpannedVec<StructField>,
    },
    Enumeration {
        name: String,
        items: SpannedVec<EnumItem>,
    },
    Union {
        name: String,
        fields: SpannedVec<StructField>,
    },
}

impl Item {
    pub fn make_item(self, visibility: Visibility, span: Span) -> ASTNode {
        ASTNode::Item {
            visibility,
            item: Spanned { node: self, span },
        }
    }
}

impl NodeMaker for Item {
    fn make_node(self, span: Span) -> ASTNode {
        ASTNode::Item {
            visibility: Visibility::Private,
            item: Spanned { node: self, span },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EnumItem {
    StructItem {
        visibility: Visibility,
        name: String,
        fields: SpannedVec<StructField>,
    },
    TupleItem {
        visibility: Visibility,
        name: String,
    },
}

impl NodeMaker for EnumItem {
    fn make_node(self, span: Span) -> ASTNode {
        ASTNode::EnumItem(Spanned { node: self, span })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StructField {
    pub visibility: Visibility,
    pub name: String,
    pub field_type: Spanned<TypeKind>,
}

impl NodeMaker for StructField {
    fn make_node(self, span: Span) -> ASTNode {
        ASTNode::StructField(Spanned { node: self, span })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Expression {
    Literal(Spanned<Literal>),
    Path(Spanned<Path>),
    Binary {
        operator: BinaryOperator,
        left: SpannedBox<Expression>,
        right: SpannedBox<Expression>,
    },
    Assign {
        operator: AssignOperator,
        left: SpannedBox<Expression>,
        right: SpannedBox<Expression>,
    },
    Comparison {
        operator: ComparisonOperator,
        left: SpannedBox<Expression>,
        right: SpannedBox<Expression>,
    },
    Logical {
        operator: LogicalOperator,
        left: SpannedBox<Expression>,
        right: SpannedBox<Expression>,
    },
    Array {
        elements: ArrayElements,
    },
    Call {
        function: SpannedBox<Expression>,
        arguments: Vec<SpannedBox<Expression>>,
    },
    FieldAccess {
        parent: SpannedBox<Expression>,
        field_name: String,
    },
    MethodCall(MethodCall),
    Index {
        parent: SpannedBox<Expression>,
        index: SpannedBox<Expression>,
    },
    Block(SpannedVec<Statement>),
    Loop(Spanned<LoopExpr>),
    If {
        condition: SpannedBox<Expression>,
        then_body: SpannedBox<Expression>,
        else_body: Option<SpannedBox<Expression>>,
    },
    Continue {
        label: Option<String>,
    },
    Break {
        label: Option<String>,
        expression: Option<SpannedBox<Expression>>,
    },
    Return(Option<SpannedBox<Expression>>),
}

impl NodeMaker for Expression {
    fn make_node(self, span: Span) -> ASTNode {
        ASTNode::Expression(Spanned {
            node: Box::new(self),
            span,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LoopExpr {
    While {
        condition: SpannedBox<Expression>,
        body: SpannedBox<Expression>,
    },
    For {
        pattern: Spanned<Pattern>,
        iterator: SpannedBox<Expression>,
        body: SpannedBox<Expression>,
    },
    Loop {
        body: SpannedBox<Expression>,
    },
}

impl NodeMaker for LoopExpr {
    fn make_node(self, span: Span) -> ASTNode {
        ASTNode::Loop(Spanned { node: self, span })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Statement {
    Semicolon,
    Expression(SpannedBox<Expression>),
    Let {
        name: Spanned<Pattern>,
        variable_type: Option<Spanned<TypeKind>>,
        label: Option<String>,
        initializer: Option<SpannedBox<Expression>>,
    },
    Item(Spanned<Item>),
}

impl NodeMaker for Statement {
    fn make_node(self, span: Span) -> ASTNode {
        ASTNode::Statement(Spanned { node: self, span })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TypeAlias {
    pub name: String,
    pub target: TypeKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TypeKind {
    // 最終的にはすべてこのPrimitiveTypeになる
    Primitive(PrimitiveType),

    // hoge::Fuga
    // Vec<T>, HashMap<K, V>
    // int, u32, String
    PathType(SpannedBox<Path>),

    // fn(i32) -> bool
    Function {
        params: SpannedVec<TypeKind>,
        return_type: SpannedBox<TypeKind>,
    },
}

impl TypeKind {
    pub fn make_single_type(type_name: String, span: Span) -> Self {
        Self::PathType(Spanned {
            node: Box::new(Path::make(type_name, span.clone())),
            span,
        })
    }
}

impl NodeMaker for TypeKind {
    fn make_node(self, span: Span) -> ASTNode {
        ASTNode::Type(Spanned { node: self, span })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FunctionParam {
    pub pattern: Option<Spanned<Pattern>>,
    pub param_type: Spanned<TypeKind>,
}

impl NodeMaker for FunctionParam {
    fn make_node(self, span: Span) -> ASTNode {
        ASTNode::FunctionParameter(Spanned { node: self, span })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ArrayElements {
    List(SpannedVec<Expression>),
    Repeat {
        value: SpannedBox<Expression>,
        count: SpannedBox<Expression>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MethodCall {
    pub name: Path,
    pub params: SpannedVec<Expression>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Literal {
    Float { value: String, value_type: TypeKind },
    Integer { value: String, value_type: TypeKind },
    Char { value: String, value_type: TypeKind },
    UnicodeChar { value: String, value_type: TypeKind },
    String { value: String, value_type: TypeKind },
    Bool(bool),
}

impl NodeMaker for Literal {
    fn make_node(self, span: Span) -> ASTNode {
        ASTNode::Expression(Spanned {
            node: Box::new(Expression::Literal(Spanned { node: self, span })),
            span,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AssignOperator {
    Assign,
    AddAssign,
    SubAssign,
    MulAssign,
    DivAssign,
    ModAssign,
    AndAssign,
    OrAssign,
    XorAssign,
    LeftShiftAssign,
    RightShiftAssign,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LogicalOperator {
    Not, // !
    And, // &&
    Or,  // ||
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Pattern {
    None,
    Multiply(SpannedVec<Pattern>),
    Literal(Literal),
    Identifier {
        ident: String,
        mutable: bool,
        reference: bool,
    },
}

impl Pattern {}

impl NodeMaker for Pattern {
    fn make_node(self, span: Span) -> ASTNode {
        ASTNode::Pattern(Spanned { node: self, span })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Path {
    pub segments: SpannedVec<PathSegment>,
}

impl Path {
    pub fn make(type_name: String, span: Span) -> Self {
        Self {
            segments: vec![Spanned {
                node: PathSegment::make(type_name),
                span,
            }],
        }
    }

    pub fn add_path(&mut self, path: Spanned<PathSegment>) {
        self.segments.push(path);
    }

    pub fn last_ident(&self) -> &str {
        self.segments
            .last()
            .map(|seg| seg.node.ident.as_str())
            .unwrap_or("")
    }
}

impl NodeMaker for Path {
    fn make_node(self, span: Span) -> ASTNode {
        ASTNode::Path(Spanned { node: self, span })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PathSegment {
    pub ident: String,
    pub arguments: SpannedVec<TypeKind>, // generic
}

impl PathSegment {
    fn make(type_name: String) -> Self {
        Self {
            ident: type_name,
            arguments: vec![],
        }
    }
}

impl NodeMaker for PathSegment {
    fn make_node(self, span: Span) -> ASTNode {
        ASTNode::PathSegment(Spanned { node: self, span })
    }
}
