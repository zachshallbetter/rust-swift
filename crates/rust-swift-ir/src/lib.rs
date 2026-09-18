//! Target-neutral semantic IR for the Rust to Swift compiler.
//!
//! Represents typed values, source spans, ownership/mutability, expressions,
//! statements, integer arithmetic modes (checked vs wrapping vs saturating),
//! and maps directly to the Compiler Semantic Graph (CSG) exchange model.

use reflective_rust_csg::{
    CompilerSemanticGraph, CsgEdge, CsgNode, CsgNodeKind, GraphProvenance, SourceSpan as CsgSpan,
};
use reflective_rust_protocol::AccessLevel;
use serde::{Deserialize, Serialize};

/// Source code location span.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceSpan {
    pub file: String,
    pub start_line: u32,
    pub end_line: u32,
}

impl From<SourceSpan> for CsgSpan {
    fn from(span: SourceSpan) -> Self {
        CsgSpan {
            file: span.file,
            start_line: span.start_line,
            end_line: span.end_line,
        }
    }
}

/// Primitive and user-defined types admitted in v0.1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Type {
    I8,
    I16,
    I32,
    I64,
    I128,
    Isize,
    U8,
    U16,
    U32,
    U64,
    U128,
    Usize,
    Bool,
    Unit,
    String,
    Option(Box<Type>),
    Result {
        ok: Box<Type>,
        err: Box<Type>,
    },
    Named(String),
    Tuple(Vec<Type>),
    Array {
        element: Box<Type>,
        size: Option<usize>,
    },
    Ref {
        inner: Box<Type>,
        mutable: bool,
    },
}

impl Type {
    pub fn is_integer(&self) -> bool {
        matches!(
            self,
            Type::I8
                | Type::I16
                | Type::I32
                | Type::I64
                | Type::I128
                | Type::Isize
                | Type::U8
                | Type::U16
                | Type::U32
                | Type::U64
                | Type::U128
                | Type::Usize
        )
    }

    pub fn is_signed_integer(&self) -> bool {
        matches!(
            self,
            Type::I8 | Type::I16 | Type::I32 | Type::I64 | Type::I128 | Type::Isize
        )
    }
}

/// Integer operation kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IntegerOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
}

/// Overflow handling mode for arithmetic operations.
/// In compatibility mode: Checked traps upon overflow; Wrapping performs two's complement wrap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IntegerMode {
    Checked,
    Wrapping,
    Saturating,
}

/// Comparison operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CmpOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

/// Logical operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LogicalOp {
    And,
    Or,
}

/// Binary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BinaryOp {
    Arithmetic { op: IntegerOp, mode: IntegerMode },
    Comparison(CmpOp),
    Logical(LogicalOp),
}

/// Unary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UnaryOp {
    Not,
    Neg,
    Deref,
}

/// Literals supported in v0.1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Literal {
    Int { value: i128, ty: Type },
    UInt { value: u128, ty: Type },
    Bool(bool),
    String(String),
    Unit,
}

/// Pattern matching constructs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Pattern {
    Wildcard,
    Literal(Literal),
    Variable(String),
    EnumVariant {
        enum_name: Option<String>,
        variant: String,
    },
    EnumVariantPayload {
        enum_name: Option<String>,
        variant: String,
        patterns: Vec<Pattern>,
    },
    Tuple(Vec<Pattern>),
    Struct {
        name: String,
        fields: Vec<(String, Pattern)>,
    },
}

/// Match arm definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub guard: Option<Expr>,
    pub body: Expr,
}

/// Expressions in the Semantic IR.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExprKind {
    Literal(Literal),
    Variable(String),
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    Call {
        func: String,
        args: Vec<Expr>,
    },
    MethodCall {
        receiver: Box<Expr>,
        method: String,
        args: Vec<Expr>,
    },
    FieldAccess {
        receiver: Box<Expr>,
        field: String,
    },
    StructInit {
        name: String,
        fields: Vec<(String, Expr)>,
    },
    Block {
        stmts: Vec<Stmt>,
        expr: Option<Box<Expr>>,
    },
    If {
        cond: Box<Expr>,
        then_branch: Box<Expr>,
        else_branch: Option<Box<Expr>>,
    },
    Match {
        expr: Box<Expr>,
        arms: Vec<MatchArm>,
    },
    Loop {
        body: Box<Expr>,
    },
    While {
        cond: Box<Expr>,
        body: Box<Expr>,
    },
    Tuple(Vec<Expr>),
    Array(Vec<Expr>),
    Index {
        expr: Box<Expr>,
        index: Box<Expr>,
    },
    Slice {
        expr: Box<Expr>,
        start: Option<Box<Expr>>,
        end: Option<Box<Expr>>,
    },
    Borrow {
        expr: Box<Expr>,
        mutable: bool,
    },
    Return(Option<Box<Expr>>),
    Break(Option<Box<Expr>>),
    Continue,
    Panic(Option<String>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Expr {
    pub kind: ExprKind,
    pub ty: Type,
}

impl Expr {
    pub fn new(kind: ExprKind, ty: Type) -> Self {
        Self { kind, ty }
    }
}

/// Statements in the Semantic IR.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Stmt {
    Let {
        name: String,
        mutable: bool,
        ty: Option<Type>,
        init: Option<Expr>,
    },
    Expr(Expr),
    Assign {
        target: Expr,
        value: Expr,
    },
    Assert {
        condition: Expr,
        message: Option<String>,
    },
    DebugAssert {
        condition: Expr,
        message: Option<String>,
    },
    AssertEq {
        left: Expr,
        right: Expr,
        message: Option<String>,
    },
    Panic {
        message: Option<String>,
    },
}

/// Function parameter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Param {
    pub name: String,
    pub ty: Type,
    pub mutable: bool,
}

/// Function declaration and body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Function {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Type,
    pub body: Expr,
    pub is_public: bool,
    pub span: SourceSpan,
}

/// Struct field definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldDef {
    pub name: String,
    pub ty: Type,
    pub is_public: bool,
}

/// Struct definition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StructDef {
    pub name: String,
    pub fields: Vec<FieldDef>,
    pub is_public: bool,
    pub span: SourceSpan,
}

/// Variant payload data representation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VariantPayload {
    Unit,
    Tuple(Vec<Type>),
    Struct(Vec<FieldDef>),
}

/// Enum variant representation with optional payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnumVariant {
    pub name: String,
    pub payload: VariantPayload,
}

/// Enum definition supporting fieldless and payload-carrying variants.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnumDef {
    pub name: String,
    pub variants: Vec<EnumVariant>,
    pub is_public: bool,
    pub span: SourceSpan,
}

impl EnumDef {
    pub fn is_fieldless(&self) -> bool {
        self.variants
            .iter()
            .all(|v| matches!(v.payload, VariantPayload::Unit))
    }
}

impl From<&str> for EnumVariant {
    fn from(name: &str) -> Self {
        Self {
            name: name.to_string(),
            payload: VariantPayload::Unit,
        }
    }
}

impl From<String> for EnumVariant {
    fn from(name: String) -> Self {
        Self {
            name,
            payload: VariantPayload::Unit,
        }
    }
}

/// Method receiver mutability and reference mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReceiverKind {
    Value,
    Ref,
    MutRef,
}

/// Method declaration inside an impl block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Method {
    pub name: String,
    pub receiver: Option<ReceiverKind>,
    pub params: Vec<Param>,
    pub return_type: Type,
    pub body: Expr,
    pub is_public: bool,
    pub span: SourceSpan,
}

/// Inherent implementation block for a struct or enum type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImplBlock {
    pub target_type: String,
    pub methods: Vec<Method>,
    pub span: SourceSpan,
}

/// Item in a module.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Item {
    Function(Function),
    Struct(StructDef),
    Enum(EnumDef),
    Impl(ImplBlock),
}

/// Top-level compilation unit module.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Module {
    pub name: String,
    pub items: Vec<Item>,
}

impl Module {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            items: Vec::new(),
        }
    }

    /// Project the module into a CompilerSemanticGraph compliant with `reflective-rust-csg`.
    pub fn to_csg(&self, source_hash: &str) -> CompilerSemanticGraph {
        let mut nodes = Vec::new();
        let mut edges = Vec::new();

        let root_span = CsgSpan {
            file: format!("{}.rs", self.name),
            start_line: 1,
            end_line: 1,
        };

        let root_id = format!("crate::{}", self.name);
        nodes.push(CsgNode {
            id: root_id.clone(),
            name: self.name.clone(),
            kind: CsgNodeKind::Crate,
            span: root_span,
            minimum_access: AccessLevel::Public,
            size_bytes: None,
        });

        for item in &self.items {
            match item {
                Item::Function(func) => {
                    let fn_id = format!("{}::{}", root_id, func.name);
                    nodes.push(CsgNode {
                        id: fn_id.clone(),
                        name: func.name.clone(),
                        kind: CsgNodeKind::Function,
                        span: func.span.clone().into(),
                        minimum_access: if func.is_public {
                            AccessLevel::Public
                        } else {
                            AccessLevel::Owner
                        },
                        size_bytes: None,
                    });
                    edges.push(CsgEdge {
                        from: root_id.clone(),
                        to: fn_id,
                        relationship: "contains_item".into(),
                        minimum_access: AccessLevel::Public,
                    });
                }
                Item::Struct(strct) => {
                    let struct_id = format!("{}::{}", root_id, strct.name);
                    nodes.push(CsgNode {
                        id: struct_id.clone(),
                        name: strct.name.clone(),
                        kind: CsgNodeKind::Struct,
                        span: strct.span.clone().into(),
                        minimum_access: if strct.is_public {
                            AccessLevel::Public
                        } else {
                            AccessLevel::Owner
                        },
                        size_bytes: None,
                    });
                    edges.push(CsgEdge {
                        from: root_id.clone(),
                        to: struct_id.clone(),
                        relationship: "contains_item".into(),
                        minimum_access: AccessLevel::Public,
                    });

                    for field in &strct.fields {
                        let field_id = format!("{}::{}", struct_id, field.name);
                        nodes.push(CsgNode {
                            id: field_id.clone(),
                            name: field.name.clone(),
                            kind: CsgNodeKind::Field,
                            span: strct.span.clone().into(),
                            minimum_access: if field.is_public {
                                AccessLevel::Public
                            } else {
                                AccessLevel::Owner
                            },
                            size_bytes: None,
                        });
                        edges.push(CsgEdge {
                            from: struct_id.clone(),
                            to: field_id,
                            relationship: "contains_field".into(),
                            minimum_access: AccessLevel::Public,
                        });
                    }
                }
                Item::Enum(enm) => {
                    let enum_id = format!("{}::{}", root_id, enm.name);
                    nodes.push(CsgNode {
                        id: enum_id.clone(),
                        name: enm.name.clone(),
                        kind: CsgNodeKind::Enum,
                        span: enm.span.clone().into(),
                        minimum_access: if enm.is_public {
                            AccessLevel::Public
                        } else {
                            AccessLevel::Owner
                        },
                        size_bytes: None,
                    });
                    edges.push(CsgEdge {
                        from: root_id.clone(),
                        to: enum_id.clone(),
                        relationship: "contains_item".into(),
                        minimum_access: AccessLevel::Public,
                    });

                    for variant in &enm.variants {
                        let variant_id = format!("{}::{}", enum_id, variant.name);
                        nodes.push(CsgNode {
                            id: variant_id.clone(),
                            name: variant.name.clone(),
                            kind: CsgNodeKind::Field,
                            span: enm.span.clone().into(),
                            minimum_access: if enm.is_public {
                                AccessLevel::Public
                            } else {
                                AccessLevel::Owner
                            },
                            size_bytes: None,
                        });
                        edges.push(CsgEdge {
                            from: enum_id.clone(),
                            to: variant_id,
                            relationship: "contains_variant".into(),
                            minimum_access: AccessLevel::Public,
                        });
                    }
                }
                Item::Impl(imp) => {
                    let impl_id = format!("{}::{}", root_id, imp.target_type);
                    for method in &imp.methods {
                        let method_id = format!("{}::{}", impl_id, method.name);
                        nodes.push(CsgNode {
                            id: method_id.clone(),
                            name: method.name.clone(),
                            kind: CsgNodeKind::Function,
                            span: method.span.clone().into(),
                            minimum_access: if method.is_public {
                                AccessLevel::Public
                            } else {
                                AccessLevel::Owner
                            },
                            size_bytes: None,
                        });
                        edges.push(CsgEdge {
                            from: root_id.clone(),
                            to: method_id,
                            relationship: "contains_item".into(),
                            minimum_access: AccessLevel::Public,
                        });
                    }
                }
            }
        }

        let formatted_hash = if source_hash.starts_with("sha256:") {
            source_hash.to_string()
        } else {
            format!("sha256:{}", source_hash)
        };

        CompilerSemanticGraph {
            schema_version: reflective_rust_csg::CSG_SCHEMA_VERSION.into(),
            provenance: GraphProvenance::Compiler {
                compiler: "rust-swift-compiler".into(),
                backend: "swift".into(),
                extractor: "rust-swift-frontend".into(),
                source_hash: formatted_hash,
            },
            nodes,
            edges,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_csg_projection_validates_successfully() {
        let mut module = Module::new("test_mod");
        module.items.push(Item::Function(Function {
            name: "calculate".into(),
            params: vec![Param {
                name: "x".into(),
                ty: Type::I32,
                mutable: false,
            }],
            return_type: Type::I32,
            body: Expr::new(
                ExprKind::Literal(Literal::Int {
                    value: 42,
                    ty: Type::I32,
                }),
                Type::I32,
            ),
            is_public: true,
            span: SourceSpan {
                file: "test_mod.rs".into(),
                start_line: 1,
                end_line: 10,
            },
        }));

        let csg = module.to_csg("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef");
        assert!(csg.provenance.is_compiler_certified());
        assert!(csg.validate().is_ok());
    }
}
