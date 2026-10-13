//! Frontend parser, semantic analyzer, and lowering pass for the Rust-to-Swift compiler.
//!
//! Enforces the Language Matrix rejection policy, emits stable diagnostic codes,
//! and lowers admitted Rust syntax trees into target-neutral Semantic IR.

pub mod diagnostic;

use diagnostic::{Diagnostic, DiagnosticCode};
use rust_swift_ir::{
    BinaryOp, CmpOp, EnumDef, Expr, ExprKind, FieldDef, Function, IntegerMode, IntegerOp,
    Item as IrItem, Literal, LogicalOp, MatchArm, Module, Param, Pattern, SourceSpan, Stmt,
    StructDef, Type, UnaryOp,
};
use syn::spanned::Spanned;

pub struct Frontend {
    file_name: String,
    diagnostics: Vec<Diagnostic>,
}

impl Frontend {
    pub fn new(file_name: impl Into<String>) -> Self {
        Self {
            file_name: file_name.into(),
            diagnostics: Vec::new(),
        }
    }

    fn make_span<S: Spanned>(&self, item: &S) -> SourceSpan {
        let span = item.span();
        SourceSpan {
            file: self.file_name.clone(),
            start_line: span.start().line as u32,
            end_line: span.end().line as u32,
        }
    }

    fn reject<S: Spanned>(
        &mut self,
        code: DiagnosticCode,
        message: impl Into<String>,
        item: &S,
        alt: Option<&str>,
    ) {
        self.diagnostics.push(Diagnostic::rejected(
            code,
            message,
            self.make_span(item),
            alt.map(|s| s.to_string()),
        ));
    }

    pub fn parse_and_lower(&mut self, source: &str) -> Result<Module, Vec<Diagnostic>> {
        let syn_file = match syn::parse_str::<syn::File>(source) {
            Ok(file) => file,
            Err(err) => {
                let span = SourceSpan {
                    file: self.file_name.clone(),
                    start_line: err.span().start().line as u32,
                    end_line: err.span().end().line as u32,
                };
                self.diagnostics.push(Diagnostic::rejected(
                    DiagnosticCode::SyntaxError,
                    format!("Rust syntax error: {}", err),
                    span,
                    None,
                ));
                return Err(self.diagnostics.clone());
            }
        };

        let mod_name = std::path::Path::new(&self.file_name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("module");

        let mut module = Module::new(mod_name);

        for item in &syn_file.items {
            if let Some(ir_item) = self.lower_item(item) {
                module.items.push(ir_item);
            }
        }

        if !self.diagnostics.is_empty() {
            Err(self.diagnostics.clone())
        } else {
            Ok(module)
        }
    }

    fn lower_item(&mut self, item: &syn::Item) -> Option<IrItem> {
        match item {
            syn::Item::Fn(item_fn) => self.lower_fn(item_fn).map(IrItem::Function),
            syn::Item::Struct(item_struct) => {
                self.lower_struct(item_struct).map(IrItem::Struct)
            }
            syn::Item::Enum(item_enum) => self.lower_enum(item_enum).map(IrItem::Enum),
            syn::Item::Macro(m) => {
                self.reject(
                    DiagnosticCode::MacroRejected,
                    "Macro definitions and unexpanded macros are rejected in v0.1",
                    m,
                    Some("Expand macros before compilation"),
                );
                None
            }
            syn::Item::Trait(t) => {
                self.reject(
                    DiagnosticCode::ComplexTraitRejected,
                    "Trait declarations are rejected in v0.1",
                    t,
                    Some("Use concrete functions and structs"),
                );
                None
            }
            syn::Item::Impl(imp) => {
                if imp.trait_.is_some() {
                    self.reject(
                        DiagnosticCode::ComplexTraitRejected,
                        "Trait implementations are rejected in v0.1",
                        imp,
                        Some("Use standalone pure functions or inherent methods"),
                    );
                    None
                } else {
                    for item in &imp.items {
                        if let syn::ImplItem::Fn(method) = item {
                            if let Some(func) = self.lower_impl_fn(&imp.self_ty, method) {
                                return Some(IrItem::Function(func));
                            }
                        }
                    }
                    None
                }
            }
            other => {
                self.reject(
                    DiagnosticCode::UnsupportedTypeRejected,
                    "Unsupported top-level item in v0.1",
                    other,
                    None,
                );
                None
            }
        }
    }

    fn lower_fn(&mut self, item_fn: &syn::ItemFn) -> Option<Function> {
        if item_fn.sig.asyncness.is_some() {
            self.reject(
                DiagnosticCode::AsyncRejected,
                "Async functions are rejected in v0.1",
                &item_fn.sig.fn_token,
                Some("Use synchronous execution"),
            );
            return None;
        }

        if item_fn.sig.unsafety.is_some() {
            self.reject(
                DiagnosticCode::UnsafeRejected,
                "Unsafe functions are rejected in v0.1",
                &item_fn.sig.unsafety,
                Some("Implement using safe Rust constructs"),
            );
            return None;
        }

        let is_public = matches!(item_fn.vis, syn::Visibility::Public(_));
        let name = item_fn.sig.ident.to_string();

        let mut params = Vec::new();
        for input in &item_fn.sig.inputs {
            match input {
                syn::FnArg::Receiver(r) => {
                    self.reject(
                        DiagnosticCode::UnsupportedTypeRejected,
                        "Method receiver 'self' in free functions is not supported",
                        r,
                        None,
                    );
                }
                syn::FnArg::Typed(pat_type) => {
                    let param_name = match &*pat_type.pat {
                        syn::Pat::Ident(id) => id.ident.to_string(),
                        other => {
                            self.reject(
                                DiagnosticCode::UnsupportedTypeRejected,
                                "Complex pattern parameter not supported",
                                other,
                                Some("Use simple identifier parameter"),
                            );
                            "arg".into()
                        }
                    };
                    let mutable = match &*pat_type.pat {
                        syn::Pat::Ident(id) => id.mutability.is_some(),
                        _ => false,
                    };
                    let ty = self.lower_type(&pat_type.ty);
                    params.push(Param {
                        name: param_name,
                        ty,
                        mutable,
                    });
                }
            }
        }

        let return_type = match &item_fn.sig.output {
            syn::ReturnType::Default => Type::Unit,
            syn::ReturnType::Type(_, ty) => self.lower_type(ty),
        };

        let body = self.lower_block(&item_fn.block);

        Some(Function {
            name,
            params,
            return_type,
            body,
            is_public,
            span: self.make_span(item_fn),
        })
    }

    fn lower_impl_fn(&mut self, self_ty: &syn::Type, method: &syn::ImplItemFn) -> Option<Function> {
        let type_prefix = match self_ty {
            syn::Type::Path(p) => p
                .path
                .segments
                .last()
                .map(|s| s.ident.to_string())
                .unwrap_or_else(|| "Self".into()),
            _ => "Self".into(),
        };

        let fn_name = format!("{}_{}", type_prefix, method.sig.ident);
        let is_public = matches!(method.vis, syn::Visibility::Public(_));

        let mut params = Vec::new();
        for input in &method.sig.inputs {
            match input {
                syn::FnArg::Receiver(r) => {
                    let self_t = self.lower_type(self_ty);
                    let ty = if r.reference.is_some() {
                        Type::Ref {
                            inner: Box::new(self_t),
                            mutable: r.mutability.is_some(),
                        }
                    } else {
                        self_t
                    };
                    params.push(Param {
                        name: "self".into(),
                        ty,
                        mutable: r.mutability.is_some(),
                    });
                }
                syn::FnArg::Typed(pat_type) => {
                    let param_name = match &*pat_type.pat {
                        syn::Pat::Ident(id) => id.ident.to_string(),
                        _ => "arg".into(),
                    };
                    let ty = self.lower_type(&pat_type.ty);
                    params.push(Param {
                        name: param_name,
                        ty,
                        mutable: false,
                    });
                }
            }
        }

        let return_type = match &method.sig.output {
            syn::ReturnType::Default => Type::Unit,
            syn::ReturnType::Type(_, ty) => self.lower_type(ty),
        };

        let body = self.lower_block(&method.block);

        Some(Function {
            name: fn_name,
            params,
            return_type,
            body,
            is_public,
            span: self.make_span(method),
        })
    }

    fn lower_struct(&mut self, item_struct: &syn::ItemStruct) -> Option<StructDef> {
        let name = item_struct.ident.to_string();
        let is_public = matches!(item_struct.vis, syn::Visibility::Public(_));
        let mut fields = Vec::new();

        match &item_struct.fields {
            syn::Fields::Named(named) => {
                for field in &named.named {
                    let field_name = field.ident.as_ref().map(|i| i.to_string()).unwrap();
                    let ty = self.lower_type(&field.ty);
                    let field_pub = matches!(field.vis, syn::Visibility::Public(_));
                    fields.push(FieldDef {
                        name: field_name,
                        ty,
                        is_public: field_pub,
                    });
                }
            }
            syn::Fields::Unnamed(unnamed) => {
                self.reject(
                    DiagnosticCode::UnsupportedTypeRejected,
                    "Tuple structs are not supported in v0.1",
                    unnamed,
                    Some("Use structs with named fields"),
                );
            }
            syn::Fields::Unit => {}
        }

        Some(StructDef {
            name,
            fields,
            is_public,
            span: self.make_span(item_struct),
        })
    }

    fn lower_enum(&mut self, item_enum: &syn::ItemEnum) -> Option<EnumDef> {
        let name = item_enum.ident.to_string();
        let is_public = matches!(item_enum.vis, syn::Visibility::Public(_));
        let mut variants = Vec::new();

        for variant in &item_enum.variants {
            if !matches!(variant.fields, syn::Fields::Unit) {
                self.reject(
                    DiagnosticCode::UnsupportedTypeRejected,
                    "Enum variants with payloads are deferred to v0.2",
                    variant,
                    Some("Use fieldless enums in v0.1"),
                );
            }
            variants.push(variant.ident.to_string());
        }

        Some(EnumDef {
            name,
            variants,
            is_public,
            span: self.make_span(item_enum),
        })
    }

    pub fn lower_type(&mut self, ty: &syn::Type) -> Type {
        match ty {
            syn::Type::Path(type_path) => {
                if let Some(segment) = type_path.path.segments.last() {
                    let ident_str = segment.ident.to_string();
                    match ident_str.as_str() {
                        "i8" => Type::I8,
                        "i16" => Type::I16,
                        "i32" => Type::I32,
                        "i64" => Type::I64,
                        "i128" => Type::I128,
                        "isize" => Type::Isize,
                        "u8" => Type::U8,
                        "u16" => Type::U16,
                        "u32" => Type::U32,
                        "u64" => Type::U64,
                        "u128" => Type::U128,
                        "usize" => Type::Usize,
                        "bool" => Type::Bool,
                        name => Type::Named(name.into()),
                    }
                } else {
                    Type::Unit
                }
            }
            syn::Type::Tuple(t) if t.elems.is_empty() => Type::Unit,
            syn::Type::Tuple(t) => {
                let elems = t.elems.iter().map(|e| self.lower_type(e)).collect();
                Type::Tuple(elems)
            }
            syn::Type::Reference(r) => {
                let inner = self.lower_type(&r.elem);
                Type::Ref {
                    inner: Box::new(inner),
                    mutable: r.mutability.is_some(),
                }
            }
            syn::Type::Ptr(p) => {
                self.reject(
                    DiagnosticCode::RawPointerRejected,
                    "Raw pointers are rejected in v0.1",
                    p,
                    Some("Use safe references or values"),
                );
                Type::Unit
            }
            syn::Type::TraitObject(t) => {
                self.reject(
                    DiagnosticCode::DynamicDispatchRejected,
                    "Dynamic dispatch / trait objects are rejected in v0.1",
                    t,
                    Some("Use concrete types"),
                );
                Type::Unit
            }
            other => {
                self.reject(
                    DiagnosticCode::UnsupportedTypeRejected,
                    "Unsupported type in v0.1",
                    other,
                    None,
                );
                Type::Unit
            }
        }
    }

    fn lower_block(&mut self, block: &syn::Block) -> Expr {
        let mut stmts = Vec::new();
        let mut final_expr = None;

        for (idx, stmt) in block.stmts.iter().enumerate() {
            let is_last = idx == block.stmts.len() - 1;
            match stmt {
                syn::Stmt::Local(local) => {
                    let (name, mutable) = match &local.pat {
                        syn::Pat::Ident(id) => (id.ident.to_string(), id.mutability.is_some()),
                        other => {
                            self.reject(
                                DiagnosticCode::UnsupportedTypeRejected,
                                "Complex pattern in let binding not supported",
                                other,
                                None,
                            );
                            ("val".into(), false)
                        }
                    };
                    let ty = None;
                    let init = local
                        .init
                        .as_ref()
                        .map(|init| self.lower_expr(&init.expr));
                    stmts.push(Stmt::Let {
                        name,
                        mutable,
                        ty,
                        init,
                    });
                }
                syn::Stmt::Expr(expr, semi) => {
                    if semi.is_some() || !is_last {
                        match expr {
                            syn::Expr::Assign(assign) => {
                                let target = self.lower_expr(&assign.left);
                                let value = self.lower_expr(&assign.right);
                                stmts.push(Stmt::Assign { target, value });
                            }
                            _ => {
                                let ir_expr = self.lower_expr(expr);
                                stmts.push(Stmt::Expr(ir_expr));
                            }
                        }
                    } else {
                        final_expr = Some(Box::new(self.lower_expr(expr)));
                    }
                }
                syn::Stmt::Item(item) => {
                    self.reject(
                        DiagnosticCode::UnsupportedTypeRejected,
                        "Nested items inside blocks are not supported",
                        item,
                        Some("Declare items at file scope"),
                    );
                }
                syn::Stmt::Macro(m) => {
                    self.reject(
                        DiagnosticCode::MacroRejected,
                        "Macros inside statements are rejected in v0.1",
                        m,
                        None,
                    );
                }
            }
        }

        let ty = final_expr
            .as_ref()
            .map(|e| e.ty.clone())
            .unwrap_or(Type::Unit);

        Expr::new(ExprKind::Block { stmts, expr: final_expr }, ty)
    }

    pub fn lower_expr(&mut self, expr: &syn::Expr) -> Expr {
        match expr {
            syn::Expr::Lit(expr_lit) => self.lower_lit(&expr_lit.lit),
            syn::Expr::Path(expr_path) => {
                let segments: Vec<String> = expr_path
                    .path
                    .segments
                    .iter()
                    .map(|s| s.ident.to_string())
                    .collect();
                let full_name = segments.join(".");
                Expr::new(ExprKind::Variable(full_name), Type::Unit)
            }
            syn::Expr::Binary(bin) => {
                let left = Box::new(self.lower_expr(&bin.left));
                let right = Box::new(self.lower_expr(&bin.right));
                let ty = left.ty.clone();

                let op = match bin.op {
                    syn::BinOp::Add(_) => {
                        BinaryOp::Arithmetic { op: IntegerOp::Add, mode: IntegerMode::Checked }
                    }
                    syn::BinOp::Sub(_) => {
                        BinaryOp::Arithmetic { op: IntegerOp::Sub, mode: IntegerMode::Checked }
                    }
                    syn::BinOp::Mul(_) => {
                        BinaryOp::Arithmetic { op: IntegerOp::Mul, mode: IntegerMode::Checked }
                    }
                    syn::BinOp::Div(_) => {
                        BinaryOp::Arithmetic { op: IntegerOp::Div, mode: IntegerMode::Checked }
                    }
                    syn::BinOp::Rem(_) => {
                        BinaryOp::Arithmetic { op: IntegerOp::Rem, mode: IntegerMode::Checked }
                    }
                    syn::BinOp::BitAnd(_) => {
                        BinaryOp::Arithmetic { op: IntegerOp::BitAnd, mode: IntegerMode::Checked }
                    }
                    syn::BinOp::BitOr(_) => {
                        BinaryOp::Arithmetic { op: IntegerOp::BitOr, mode: IntegerMode::Checked }
                    }
                    syn::BinOp::BitXor(_) => {
                        BinaryOp::Arithmetic { op: IntegerOp::BitXor, mode: IntegerMode::Checked }
                    }
                    syn::BinOp::Shl(_) => {
                        BinaryOp::Arithmetic { op: IntegerOp::Shl, mode: IntegerMode::Checked }
                    }
                    syn::BinOp::Shr(_) => {
                        BinaryOp::Arithmetic { op: IntegerOp::Shr, mode: IntegerMode::Checked }
                    }
                    syn::BinOp::Eq(_) => BinaryOp::Comparison(CmpOp::Eq),
                    syn::BinOp::Ne(_) => BinaryOp::Comparison(CmpOp::Ne),
                    syn::BinOp::Lt(_) => BinaryOp::Comparison(CmpOp::Lt),
                    syn::BinOp::Le(_) => BinaryOp::Comparison(CmpOp::Le),
                    syn::BinOp::Gt(_) => BinaryOp::Comparison(CmpOp::Gt),
                    syn::BinOp::Ge(_) => BinaryOp::Comparison(CmpOp::Ge),
                    syn::BinOp::And(_) => BinaryOp::Logical(LogicalOp::And),
                    syn::BinOp::Or(_) => BinaryOp::Logical(LogicalOp::Or),
                    other => {
                        self.reject(
                            DiagnosticCode::UnsupportedTypeRejected,
                            "Unsupported binary operator in v0.1",
                            &other,
                            None,
                        );
                        BinaryOp::Logical(LogicalOp::And)
                    }
                };

                let res_ty = match &op {
                    BinaryOp::Comparison(_) | BinaryOp::Logical(_) => Type::Bool,
                    BinaryOp::Arithmetic { .. } => ty,
                };

                Expr::new(ExprKind::Binary { op, left, right }, res_ty)
            }
            syn::Expr::Unary(un) => {
                let sub = Box::new(self.lower_expr(&un.expr));
                let ty = sub.ty.clone();
                let op = match un.op {
                    syn::UnOp::Not(_) => UnaryOp::Not,
                    syn::UnOp::Neg(_) => UnaryOp::Neg,
                    syn::UnOp::Deref(_) => UnaryOp::Deref,
                    other => {
                        self.reject(
                            DiagnosticCode::UnsupportedTypeRejected,
                            "Unsupported unary operator in v0.1",
                            &other,
                            None,
                        );
                        UnaryOp::Not
                    }
                };
                Expr::new(ExprKind::Unary { op, expr: sub }, ty)
            }
            syn::Expr::MethodCall(mc) => {
                let receiver = Box::new(self.lower_expr(&mc.receiver));
                let method = mc.method.to_string();
                let args: Vec<Expr> = mc.args.iter().map(|a| self.lower_expr(a)).collect();

                match method.as_str() {
                    "wrapping_add" if args.len() == 1 => {
                        let ty = receiver.ty.clone();
                        Expr::new(
                            ExprKind::Binary {
                                op: BinaryOp::Arithmetic {
                                    op: IntegerOp::Add,
                                    mode: IntegerMode::Wrapping,
                                },
                                left: receiver,
                                right: Box::new(args.into_iter().next().unwrap()),
                            },
                            ty,
                        )
                    }
                    "wrapping_sub" if args.len() == 1 => {
                        let ty = receiver.ty.clone();
                        Expr::new(
                            ExprKind::Binary {
                                op: BinaryOp::Arithmetic {
                                    op: IntegerOp::Sub,
                                    mode: IntegerMode::Wrapping,
                                },
                                left: receiver,
                                right: Box::new(args.into_iter().next().unwrap()),
                            },
                            ty,
                        )
                    }
                    "wrapping_mul" if args.len() == 1 => {
                        let ty = receiver.ty.clone();
                        Expr::new(
                            ExprKind::Binary {
                                op: BinaryOp::Arithmetic {
                                    op: IntegerOp::Mul,
                                    mode: IntegerMode::Wrapping,
                                },
                                left: receiver,
                                right: Box::new(args.into_iter().next().unwrap()),
                            },
                            ty,
                        )
                    }
                    "saturating_add" if args.len() == 1 => {
                        let ty = receiver.ty.clone();
                        Expr::new(
                            ExprKind::Binary {
                                op: BinaryOp::Arithmetic {
                                    op: IntegerOp::Add,
                                    mode: IntegerMode::Saturating,
                                },
                                left: receiver,
                                right: Box::new(args.into_iter().next().unwrap()),
                            },
                            ty,
                        )
                    }
                    "saturating_sub" if args.len() == 1 => {
                        let ty = receiver.ty.clone();
                        Expr::new(
                            ExprKind::Binary {
                                op: BinaryOp::Arithmetic {
                                    op: IntegerOp::Sub,
                                    mode: IntegerMode::Saturating,
                                },
                                left: receiver,
                                right: Box::new(args.into_iter().next().unwrap()),
                            },
                            ty,
                        )
                    }
                    _ => Expr::new(
                        ExprKind::MethodCall {
                            receiver,
                            method,
                            args,
                        },
                        Type::Unit,
                    ),
                }
            }
            syn::Expr::Call(call) => {
                let func_name = match &*call.func {
                    syn::Expr::Path(p) => p
                        .path
                        .segments
                        .iter()
                        .map(|s| s.ident.to_string())
                        .collect::<Vec<_>>()
                        .join("."),
                    _ => "unknown_func".into(),
                };
                let args = call.args.iter().map(|a| self.lower_expr(a)).collect();
                Expr::new(ExprKind::Call { func: func_name, args }, Type::Unit)
            }
            syn::Expr::Field(field) => {
                let receiver = Box::new(self.lower_expr(&field.base));
                let field_name = match &field.member {
                    syn::Member::Named(ident) => ident.to_string(),
                    syn::Member::Unnamed(idx) => idx.index.to_string(),
                };
                Expr::new(
                    ExprKind::FieldAccess {
                        receiver,
                        field: field_name,
                    },
                    Type::Unit,
                )
            }
            syn::Expr::Struct(strct) => {
                let name = strct
                    .path
                    .segments
                    .iter()
                    .map(|s| s.ident.to_string())
                    .collect::<Vec<_>>()
                    .join(".");
                let fields = strct
                    .fields
                    .iter()
                    .map(|f| {
                        let fname = match &f.member {
                            syn::Member::Named(ident) => ident.to_string(),
                            syn::Member::Unnamed(idx) => idx.index.to_string(),
                        };
                        let val = self.lower_expr(&f.expr);
                        (fname, val)
                    })
                    .collect();
                Expr::new(
                    ExprKind::StructInit { name: name.clone(), fields },
                    Type::Named(name),
                )
            }
            syn::Expr::Block(block) => self.lower_block(&block.block),
            syn::Expr::If(expr_if) => {
                let cond = Box::new(self.lower_expr(&expr_if.cond));
                let then_branch = Box::new(self.lower_block(&expr_if.then_branch));
                let else_branch = expr_if
                    .else_branch
                    .as_ref()
                    .map(|(_, e)| Box::new(self.lower_expr(e)));
                let ty = then_branch.ty.clone();
                Expr::new(
                    ExprKind::If {
                        cond,
                        then_branch,
                        else_branch,
                    },
                    ty,
                )
            }
            syn::Expr::Match(m) => {
                let expr = Box::new(self.lower_expr(&m.expr));
                let mut arms = Vec::new();
                let mut result_ty = Type::Unit;
                for arm in &m.arms {
                    let pattern = self.lower_pattern(&arm.pat);
                    let guard = arm.guard.as_ref().map(|(_, g)| self.lower_expr(g));
                    let body = self.lower_expr(&arm.body);
                    result_ty = body.ty.clone();
                    arms.push(MatchArm {
                        pattern,
                        guard,
                        body,
                    });
                }
                Expr::new(ExprKind::Match { expr, arms }, result_ty)
            }
            syn::Expr::Loop(l) => {
                let body = Box::new(self.lower_block(&l.body));
                Expr::new(ExprKind::Loop { body }, Type::Unit)
            }
            syn::Expr::While(w) => {
                let cond = Box::new(self.lower_expr(&w.cond));
                let body = Box::new(self.lower_block(&w.body));
                Expr::new(ExprKind::While { cond, body }, Type::Unit)
            }
            syn::Expr::Return(r) => {
                let val = r.expr.as_ref().map(|e| Box::new(self.lower_expr(e)));
                Expr::new(ExprKind::Return(val), Type::Unit)
            }
            syn::Expr::Break(b) => {
                let val = b.expr.as_ref().map(|e| Box::new(self.lower_expr(e)));
                Expr::new(ExprKind::Break(val), Type::Unit)
            }
            syn::Expr::Continue(_) => Expr::new(ExprKind::Continue, Type::Unit),
            syn::Expr::Paren(p) => self.lower_expr(&p.expr),
            syn::Expr::Tuple(t) => {
                let elems = t.elems.iter().map(|e| self.lower_expr(e)).collect();
                Expr::new(ExprKind::Tuple(elems), Type::Unit)
            }
            syn::Expr::Unsafe(u) => {
                self.reject(
                    DiagnosticCode::UnsafeRejected,
                    "Unsafe blocks are rejected in v0.1",
                    &u.unsafe_token,
                    Some("Use safe equivalents"),
                );
                Expr::new(ExprKind::Literal(Literal::Unit), Type::Unit)
            }
            syn::Expr::Async(a) => {
                self.reject(
                    DiagnosticCode::AsyncRejected,
                    "Async blocks are rejected in v0.1",
                    &a.async_token,
                    Some("Use synchronous code"),
                );
                Expr::new(ExprKind::Literal(Literal::Unit), Type::Unit)
            }
            syn::Expr::Await(a) => {
                self.reject(
                    DiagnosticCode::AsyncRejected,
                    ".await is rejected in v0.1",
                    &a.dot_token,
                    Some("Use synchronous calls"),
                );
                Expr::new(ExprKind::Literal(Literal::Unit), Type::Unit)
            }
            syn::Expr::Closure(c) => {
                self.reject(
                    DiagnosticCode::ClosureBorrowRejected,
                    "Closures are deferred to post-v0.1",
                    c,
                    Some("Use regular functions"),
                );
                Expr::new(ExprKind::Literal(Literal::Unit), Type::Unit)
            }
            other => {
                self.reject(
                    DiagnosticCode::UnsupportedTypeRejected,
                    "Unsupported expression in v0.1",
                    other,
                    None,
                );
                Expr::new(ExprKind::Literal(Literal::Unit), Type::Unit)
            }
        }
    }

    fn lower_pattern(&mut self, pat: &syn::Pat) -> Pattern {
        match pat {
            syn::Pat::Wild(_) => Pattern::Wildcard,
            syn::Pat::Ident(id) => Pattern::Variable(id.ident.to_string()),
            syn::Pat::Lit(lit) => match &lit.lit {
                syn::Lit::Int(i) => {
                    let val: i128 = i.base10_parse().unwrap_or(0);
                    Pattern::Literal(Literal::Int {
                        value: val,
                        ty: Type::I32,
                    })
                }
                syn::Lit::Bool(b) => Pattern::Literal(Literal::Bool(b.value)),
                _ => Pattern::Wildcard,
            },
            syn::Pat::Path(p) => {
                let segments: Vec<String> =
                    p.path.segments.iter().map(|s| s.ident.to_string()).collect();
                if segments.len() >= 2 {
                    Pattern::EnumVariant {
                        enum_name: Some(segments[..segments.len() - 1].join(".")),
                        variant: segments.last().unwrap().clone(),
                    }
                } else if let Some(last) = segments.last() {
                    Pattern::EnumVariant {
                        enum_name: None,
                        variant: last.clone(),
                    }
                } else {
                    Pattern::Wildcard
                }
            }
            syn::Pat::Tuple(t) => {
                let pats = t.elems.iter().map(|p| self.lower_pattern(p)).collect();
                Pattern::Tuple(pats)
            }
            syn::Pat::Struct(s) => {
                let name = s
                    .path
                    .segments
                    .iter()
                    .map(|seg| seg.ident.to_string())
                    .collect::<Vec<_>>()
                    .join(".");
                let fields = s
                    .fields
                    .iter()
                    .map(|f| {
                        let fname = match &f.member {
                            syn::Member::Named(i) => i.to_string(),
                            syn::Member::Unnamed(idx) => idx.index.to_string(),
                        };
                        (fname, self.lower_pattern(&f.pat))
                    })
                    .collect();
                Pattern::Struct { name, fields }
            }
            _ => Pattern::Wildcard,
        }
    }

    fn lower_lit(&mut self, lit: &syn::Lit) -> Expr {
        match lit {
            syn::Lit::Int(i) => {
                let suffix = i.suffix();
                let ty = match suffix {
                    "i8" => Type::I8,
                    "i16" => Type::I16,
                    "i32" => Type::I32,
                    "i64" => Type::I64,
                    "i128" => Type::I128,
                    "isize" => Type::Isize,
                    "u8" => Type::U8,
                    "u16" => Type::U16,
                    "u32" => Type::U32,
                    "u64" => Type::U64,
                    "u128" => Type::U128,
                    "usize" => Type::Usize,
                    _ => Type::I32,
                };
                if ty.is_signed_integer() {
                    let val: i128 = i.base10_parse().unwrap_or(0);
                    Expr::new(ExprKind::Literal(Literal::Int { value: val, ty: ty.clone() }), ty)
                } else {
                    let val: u128 = i.base10_parse().unwrap_or(0);
                    Expr::new(ExprKind::Literal(Literal::UInt { value: val, ty: ty.clone() }), ty)
                }
            }
            syn::Lit::Bool(b) => Expr::new(
                ExprKind::Literal(Literal::Bool(b.value)),
                Type::Bool,
            ),
            _ => Expr::new(ExprKind::Literal(Literal::Unit), Type::Unit),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_admitted_finite_state_parses() {
        let src = r#"
            #[derive(Clone, Copy, PartialEq, Eq)]
            pub enum State {
                Idle,
                Active,
                Paused,
                Stopped,
            }

            pub enum Event {
                Start,
                Pause,
                Resume,
                Stop,
            }

            pub fn transition(state: State, event: Event) -> State {
                match (state, event) {
                    (State::Idle, Event::Start) => State::Active,
                    (State::Active, Event::Pause) => State::Paused,
                    (State::Paused, Event::Resume) => State::Active,
                    (State::Active, Event::Stop) => State::Stopped,
                    (State::Paused, Event::Stop) => State::Stopped,
                    _ => state,
                }
            }
        "#;

        let mut frontend = Frontend::new("fsm.rs");
        let module = frontend.parse_and_lower(src).expect("Should succeed");
        assert_eq!(module.items.len(), 3);
    }

    #[test]
    fn test_unsafe_is_rejected_with_stable_diagnostic() {
        let src = r#"
            pub fn risky() -> i32 {
                unsafe { 42 }
            }
        "#;
        let mut frontend = Frontend::new("unsafe.rs");
        let errs = frontend.parse_and_lower(src).expect_err("Should reject unsafe");
        assert!(errs.iter().any(|d| d.code == DiagnosticCode::UnsafeRejected));
    }

    #[test]
    fn test_async_is_rejected_with_stable_diagnostic() {
        let src = r#"
            pub async fn fetch() -> i32 {
                42
            }
        "#;
        let mut frontend = Frontend::new("async.rs");
        let errs = frontend.parse_and_lower(src).expect_err("Should reject async");
        assert!(errs.iter().any(|d| d.code == DiagnosticCode::AsyncRejected));
    }
}
