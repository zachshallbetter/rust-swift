//! Frontend parser, semantic analyzer, and lowering pass for the Rust-to-Swift compiler.
//!
//! Enforces the Language Matrix rejection policy, emits stable diagnostic codes,
//! and lowers admitted Rust syntax trees into target-neutral Semantic IR.

pub mod diagnostic;

use diagnostic::{Diagnostic, DiagnosticCode};
use rust_swift_ir::{
    BinaryOp, CmpOp, EnumDef, EnumVariant, Expr, ExprKind, FieldDef, Function, ImplBlock,
    IntegerMode, IntegerOp, Item as IrItem, Literal, LogicalOp, MatchArm, Method, Module, Param,
    Pattern, PropertyBindingKind, ReceiverKind, SourceSpan, Stmt, StructDef, TraitDef, TraitImpl,
    TraitMethod, Type, UnaryOp, VariantPayload, ViewDef, ViewKind, ViewModifier, ViewNode,
    ViewProperty,
};
use syn::spanned::Spanned;

pub struct Frontend {
    file_name: String,
    diagnostics: Vec<Diagnostic>,
    default_codable: bool,
    default_observable: bool,
}

impl Frontend {
    pub fn new(file_name: impl Into<String>) -> Self {
        Self {
            file_name: file_name.into(),
            diagnostics: Vec::new(),
            default_codable: false,
            default_observable: false,
        }
    }

    pub fn with_default_codable(mut self, enabled: bool) -> Self {
        self.default_codable = enabled;
        self
    }

    pub fn with_default_observable(mut self, enabled: bool) -> Self {
        self.default_observable = enabled;
        self
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
        let mut view_structs: std::collections::HashMap<String, (syn::ItemStruct, SourceSpan)> =
            std::collections::HashMap::new();
        let mut view_bodies: std::collections::HashMap<String, (syn::Block, SourceSpan)> =
            std::collections::HashMap::new();

        for item in &syn_file.items {
            if let syn::Item::Struct(s) = item {
                if Self::has_view_attr(&s.attrs) {
                    let span = self.make_span(s);
                    view_structs.insert(s.ident.to_string(), (s.clone(), span));
                    continue;
                }
            }
            if let syn::Item::Impl(imp) = item {
                let target_name = match &*imp.self_ty {
                    syn::Type::Path(p) => p
                        .path
                        .segments
                        .last()
                        .map(|s| s.ident.to_string())
                        .unwrap_or_default(),
                    _ => String::new(),
                };
                let mut has_body = false;
                for impl_item in &imp.items {
                    if let syn::ImplItem::Fn(m) = impl_item {
                        if m.sig.ident == "body" {
                            let span = self.make_span(m);
                            view_bodies.insert(target_name.clone(), (m.block.clone(), span));
                            has_body = true;
                        }
                    }
                }
                if has_body {
                    continue;
                }
            }
            if let syn::Item::Fn(f) = item {
                if Self::has_view_attr(&f.attrs) {
                    if let Some(view_def) = self.lower_view_fn(f) {
                        module.items.push(IrItem::View(view_def));
                        continue;
                    }
                }
            }

            if let Some(ir_item) = self.lower_item(item) {
                module.items.push(ir_item);
            }
        }

        for (name, (s, span)) in view_structs {
            let body_block = view_bodies.get(&name).map(|(b, _)| b);
            if let Some(view_def) = self.lower_view_struct(&s, body_block, span) {
                module.items.push(IrItem::View(view_def));
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
            syn::Item::Struct(item_struct) => self.lower_struct(item_struct).map(IrItem::Struct),
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
            syn::Item::Trait(t) => self.lower_trait(t).map(IrItem::Trait),
            syn::Item::Impl(imp) => {
                if let Some((_, trait_path, _)) = &imp.trait_ {
                    let trait_name = trait_path
                        .segments
                        .last()
                        .map(|s| s.ident.to_string())
                        .unwrap_or_default();
                    if trait_name == "Drop" {
                        self.reject(
                            DiagnosticCode::CustomDropRejected,
                            "Custom Drop implementations are rejected in v0.1",
                            imp,
                            Some("Rely on deterministic value semantics and lexical scoping"),
                        );
                        None
                    } else if !imp.generics.params.is_empty() {
                        self.reject(
                            DiagnosticCode::ComplexTraitRejected,
                            "Generic trait implementations are rejected in v0.2",
                            imp,
                            Some("Use monomorphic trait implementations"),
                        );
                        None
                    } else {
                        self.lower_trait_impl(imp, trait_name)
                            .map(IrItem::TraitImpl)
                    }
                } else {
                    let target_type = match &*imp.self_ty {
                        syn::Type::Path(p) => p
                            .path
                            .segments
                            .last()
                            .map(|s| s.ident.to_string())
                            .unwrap_or_else(|| "Target".into()),
                        _ => "Target".into(),
                    };

                    let mut methods = Vec::new();
                    for item in &imp.items {
                        if let syn::ImplItem::Fn(method) = item {
                            if let Some(m) = self.lower_method(method) {
                                methods.push(m);
                            }
                        }
                    }

                    Some(IrItem::Impl(ImplBlock {
                        target_type,
                        methods,
                        span: self.make_span(imp),
                    }))
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

        if !item_fn.sig.generics.params.is_empty() {
            self.reject(
                DiagnosticCode::UnspecializableGenericRejected,
                "Generic functions without monomorphization are rejected in v0.1",
                &item_fn.sig.generics,
                Some("Specialize with concrete types (e.g. Option<i64>, Result<u32, Error>)"),
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
                    let is_mutable_binding = match &*pat_type.pat {
                        syn::Pat::Ident(id) => id.mutability.is_some(),
                        _ => false,
                    };
                    let ty = self.lower_type(&pat_type.ty);
                    params.push(Param {
                        name: param_name,
                        ty,
                        mutable: is_mutable_binding,
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

    fn lower_method(&mut self, method: &syn::ImplItemFn) -> Option<Method> {
        let name = method.sig.ident.to_string();
        let is_public = matches!(method.vis, syn::Visibility::Public(_));

        let mut receiver = None;
        let mut params = Vec::new();

        for input in &method.sig.inputs {
            match input {
                syn::FnArg::Receiver(r) => {
                    if r.reference.is_some() {
                        if r.mutability.is_some() {
                            receiver = Some(ReceiverKind::MutRef);
                        } else {
                            receiver = Some(ReceiverKind::Ref);
                        }
                    } else {
                        receiver = Some(ReceiverKind::Value);
                    }
                }
                syn::FnArg::Typed(pat_type) => {
                    let param_name = match &*pat_type.pat {
                        syn::Pat::Ident(id) => id.ident.to_string(),
                        _ => "arg".into(),
                    };
                    let is_mutable_binding = match &*pat_type.pat {
                        syn::Pat::Ident(id) => id.mutability.is_some(),
                        _ => false,
                    };
                    let ty = self.lower_type(&pat_type.ty);
                    params.push(Param {
                        name: param_name,
                        ty,
                        mutable: is_mutable_binding,
                    });
                }
            }
        }

        let return_type = match &method.sig.output {
            syn::ReturnType::Default => Type::Unit,
            syn::ReturnType::Type(_, ty) => self.lower_type(ty),
        };

        let body = self.lower_block(&method.block);

        Some(Method {
            name,
            receiver,
            params,
            return_type,
            body,
            is_public,
            span: self.make_span(method),
        })
    }

    fn lower_trait(&mut self, item_trait: &syn::ItemTrait) -> Option<TraitDef> {
        if !item_trait.generics.params.is_empty() {
            self.reject(
                DiagnosticCode::ComplexTraitRejected,
                "Generic traits with type parameters are rejected in v0.2",
                item_trait,
                Some("Use monomorphic traits"),
            );
            return None;
        }

        let name = item_trait.ident.to_string();
        let is_public = matches!(item_trait.vis, syn::Visibility::Public(_));
        let mut methods = Vec::new();

        for item in &item_trait.items {
            match item {
                syn::TraitItem::Fn(trait_fn) => {
                    if let Some(m) = self.lower_trait_method(trait_fn) {
                        methods.push(m);
                    }
                }
                syn::TraitItem::Type(_) => {
                    self.reject(
                        DiagnosticCode::ComplexTraitRejected,
                        "Associated types in traits are rejected in v0.2",
                        item,
                        Some("Use explicit return types in trait methods"),
                    );
                    return None;
                }
                other => {
                    self.reject(
                        DiagnosticCode::ComplexTraitRejected,
                        "Unsupported item in trait definition",
                        other,
                        None,
                    );
                    return None;
                }
            }
        }

        Some(TraitDef {
            name,
            methods,
            is_public,
            span: self.make_span(item_trait),
        })
    }

    fn lower_trait_method(&mut self, method: &syn::TraitItemFn) -> Option<TraitMethod> {
        let name = method.sig.ident.to_string();

        let mut receiver = None;
        let mut params = Vec::new();

        for input in &method.sig.inputs {
            match input {
                syn::FnArg::Receiver(r) => {
                    if r.reference.is_some() {
                        if r.mutability.is_some() {
                            receiver = Some(ReceiverKind::MutRef);
                        } else {
                            receiver = Some(ReceiverKind::Ref);
                        }
                    } else {
                        receiver = Some(ReceiverKind::Value);
                    }
                }
                syn::FnArg::Typed(pat_type) => {
                    let param_name = match &*pat_type.pat {
                        syn::Pat::Ident(id) => id.ident.to_string(),
                        _ => "arg".into(),
                    };
                    let is_mutable_binding = match &*pat_type.pat {
                        syn::Pat::Ident(id) => id.mutability.is_some(),
                        _ => false,
                    };
                    let ty = self.lower_type(&pat_type.ty);
                    params.push(Param {
                        name: param_name,
                        ty,
                        mutable: is_mutable_binding,
                    });
                }
            }
        }

        let return_type = match &method.sig.output {
            syn::ReturnType::Default => Type::Unit,
            syn::ReturnType::Type(_, ty) => self.lower_type(ty),
        };

        let default_body = method.default.as_ref().map(|b| self.lower_block(b));

        Some(TraitMethod {
            name,
            receiver,
            params,
            return_type,
            default_body,
            span: self.make_span(method),
        })
    }

    fn lower_trait_impl(&mut self, imp: &syn::ItemImpl, trait_name: String) -> Option<TraitImpl> {
        let target_type = match &*imp.self_ty {
            syn::Type::Path(p) => p
                .path
                .segments
                .last()
                .map(|s| s.ident.to_string())
                .unwrap_or_else(|| "Target".into()),
            _ => "Target".into(),
        };

        let mut methods = Vec::new();
        for item in &imp.items {
            if let syn::ImplItem::Fn(method) = item {
                if let Some(m) = self.lower_method(method) {
                    methods.push(m);
                }
            }
        }

        Some(TraitImpl {
            trait_name,
            target_type,
            methods,
            span: self.make_span(imp),
        })
    }

    fn has_codable_derive(attrs: &[syn::Attribute]) -> bool {
        attrs.iter().any(|attr| {
            if attr.path().is_ident("derive") {
                let mut found = false;
                let _ = attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("Serialize") || meta.path.is_ident("Deserialize") {
                        found = true;
                    }
                    Ok(())
                });
                found
            } else {
                false
            }
        })
    }

    fn has_observable_attr(attrs: &[syn::Attribute]) -> bool {
        attrs.iter().any(|attr| {
            if attr.path().is_ident("observable") {
                return true;
            }
            if attr.path().is_ident("derive") {
                let mut found = false;
                let _ = attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("Observable") {
                        found = true;
                    }
                    Ok(())
                });
                found
            } else {
                false
            }
        })
    }

    fn has_view_attr(attrs: &[syn::Attribute]) -> bool {
        attrs.iter().any(|attr| {
            if attr.path().is_ident("view") || attr.path().is_ident("swift_view") {
                return true;
            }
            if attr.path().is_ident("derive") {
                let mut found = false;
                let _ = attr.parse_nested_meta(|meta| {
                    if meta.path.is_ident("View") || meta.path.is_ident("SwiftView") {
                        found = true;
                    }
                    Ok(())
                });
                found
            } else {
                false
            }
        })
    }

    fn lower_view_struct(
        &mut self,
        s: &syn::ItemStruct,
        body_block: Option<&syn::Block>,
        span: SourceSpan,
    ) -> Option<ViewDef> {
        let name = s.ident.to_string();
        let is_public = matches!(s.vis, syn::Visibility::Public(_));
        let mut properties = Vec::new();

        if let syn::Fields::Named(named) = &s.fields {
            for field in &named.named {
                let prop_name = field.ident.as_ref().map(|i| i.to_string()).unwrap();
                let ty = self.lower_type(&field.ty);
                let is_pub = matches!(field.vis, syn::Visibility::Public(_));
                let kind = if field.attrs.iter().any(|a| a.path().is_ident("binding")) {
                    PropertyBindingKind::Binding
                } else if field.attrs.iter().any(|a| a.path().is_ident("state")) {
                    PropertyBindingKind::State
                } else if field.attrs.iter().any(|a| a.path().is_ident("observable"))
                    || matches!(ty, Type::Named(_))
                {
                    PropertyBindingKind::Observable
                } else {
                    PropertyBindingKind::Plain
                };
                properties.push(ViewProperty {
                    name: prop_name,
                    ty,
                    kind,
                    is_public: is_pub,
                });
            }
        }

        let root = if let Some(blk) = body_block {
            let syn_expr = syn::Expr::Block(syn::ExprBlock {
                attrs: Vec::new(),
                label: None,
                block: blk.clone(),
            });
            self.lower_view_expr(&syn_expr).unwrap_or_else(|| ViewNode {
                kind: ViewKind::VStack { spacing: None },
                children: Vec::new(),
                modifiers: Vec::new(),
                span: span.clone(),
            })
        } else {
            ViewNode {
                kind: ViewKind::VStack { spacing: None },
                children: Vec::new(),
                modifiers: Vec::new(),
                span: span.clone(),
            }
        };

        Some(ViewDef {
            name,
            properties,
            root,
            is_public,
            span,
        })
    }

    fn lower_view_fn(&mut self, f: &syn::ItemFn) -> Option<ViewDef> {
        let name = f.sig.ident.to_string();
        let is_public = matches!(f.vis, syn::Visibility::Public(_));
        let span = self.make_span(f);
        let mut properties = Vec::new();

        for input in &f.sig.inputs {
            if let syn::FnArg::Typed(pat_type) = input {
                if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                    let prop_name = pat_ident.ident.to_string();
                    let ty = self.lower_type(&pat_type.ty);
                    let kind = if matches!(ty, Type::Ref { mutable: true, .. }) {
                        PropertyBindingKind::Binding
                    } else if matches!(ty, Type::Named(_)) {
                        PropertyBindingKind::Observable
                    } else {
                        PropertyBindingKind::Plain
                    };
                    properties.push(ViewProperty {
                        name: prop_name,
                        ty,
                        kind,
                        is_public: true,
                    });
                }
            }
        }

        let syn_expr = syn::Expr::Block(syn::ExprBlock {
            attrs: Vec::new(),
            label: None,
            block: (*f.block).clone(),
        });

        let root = self.lower_view_expr(&syn_expr).unwrap_or_else(|| ViewNode {
            kind: ViewKind::VStack { spacing: None },
            children: Vec::new(),
            modifiers: Vec::new(),
            span: span.clone(),
        });

        Some(ViewDef {
            name,
            properties,
            root,
            is_public,
            span,
        })
    }

    fn lower_view_expr(&mut self, expr: &syn::Expr) -> Option<ViewNode> {
        let span = self.make_span(expr);
        match expr {
            syn::Expr::Call(call) => {
                let func_name = match &*call.func {
                    syn::Expr::Path(p) => p.path.segments.last().map(|s| s.ident.to_string())?,
                    _ => return None,
                };
                let kind_str = func_name.as_str();
                match kind_str {
                    "VStack" | "vstack" => {
                        let mut spacing = None;
                        let mut children = Vec::new();
                        for arg in &call.args {
                            if let syn::Expr::Lit(syn::ExprLit {
                                lit: syn::Lit::Int(i),
                                ..
                            }) = arg
                            {
                                spacing = i.base10_parse::<i32>().ok();
                            } else {
                                children.extend(self.lower_view_children(arg));
                            }
                        }
                        Some(ViewNode {
                            kind: ViewKind::VStack { spacing },
                            children,
                            modifiers: Vec::new(),
                            span,
                        })
                    }
                    "HStack" | "hstack" => {
                        let mut spacing = None;
                        let mut children = Vec::new();
                        for arg in &call.args {
                            if let syn::Expr::Lit(syn::ExprLit {
                                lit: syn::Lit::Int(i),
                                ..
                            }) = arg
                            {
                                spacing = i.base10_parse::<i32>().ok();
                            } else {
                                children.extend(self.lower_view_children(arg));
                            }
                        }
                        Some(ViewNode {
                            kind: ViewKind::HStack { spacing },
                            children,
                            modifiers: Vec::new(),
                            span,
                        })
                    }
                    "ZStack" | "zstack" => {
                        let mut children = Vec::new();
                        for arg in &call.args {
                            children.extend(self.lower_view_children(arg));
                        }
                        Some(ViewNode {
                            kind: ViewKind::ZStack,
                            children,
                            modifiers: Vec::new(),
                            span,
                        })
                    }
                    "Text" | "text" => {
                        let text_val = call
                            .args
                            .first()
                            .map(|a| self.extract_string_or_ident(a))
                            .unwrap_or_default();
                        Some(ViewNode {
                            kind: ViewKind::Text(text_val),
                            children: Vec::new(),
                            modifiers: Vec::new(),
                            span,
                        })
                    }
                    "Button" | "button" => {
                        let label = call
                            .args
                            .first()
                            .map(|a| self.extract_string_or_ident(a))
                            .unwrap_or_default();
                        let action = call
                            .args
                            .iter()
                            .nth(1)
                            .map(|a| self.extract_string_or_ident(a))
                            .unwrap_or_default();
                        Some(ViewNode {
                            kind: ViewKind::Button { label, action },
                            children: Vec::new(),
                            modifiers: Vec::new(),
                            span,
                        })
                    }
                    "Toggle" | "toggle" => {
                        let label = call
                            .args
                            .first()
                            .map(|a| self.extract_string_or_ident(a))
                            .unwrap_or_default();
                        let binding = call
                            .args
                            .iter()
                            .nth(1)
                            .map(|a| self.extract_string_or_ident(a))
                            .unwrap_or_default();
                        Some(ViewNode {
                            kind: ViewKind::Toggle { label, binding },
                            children: Vec::new(),
                            modifiers: Vec::new(),
                            span,
                        })
                    }
                    "List" | "list" => {
                        let mut children = Vec::new();
                        for arg in &call.args {
                            children.extend(self.lower_view_children(arg));
                        }
                        Some(ViewNode {
                            kind: ViewKind::List,
                            children,
                            modifiers: Vec::new(),
                            span,
                        })
                    }
                    "Spacer" | "spacer" => Some(ViewNode {
                        kind: ViewKind::Spacer,
                        children: Vec::new(),
                        modifiers: Vec::new(),
                        span,
                    }),
                    "Divider" | "divider" => Some(ViewNode {
                        kind: ViewKind::Divider,
                        children: Vec::new(),
                        modifiers: Vec::new(),
                        span,
                    }),
                    "ForEach" | "foreach" => {
                        let items = call
                            .args
                            .first()
                            .map(|a| self.extract_string_or_ident(a))
                            .unwrap_or_default();
                        let id = call
                            .args
                            .iter()
                            .nth(1)
                            .map(|a| self.extract_string_or_ident(a));
                        let mut children = Vec::new();
                        if let Some(arg) = call.args.iter().nth(2) {
                            children.extend(self.lower_view_children(arg));
                        }
                        Some(ViewNode {
                            kind: ViewKind::ForEach { items, id },
                            children,
                            modifiers: Vec::new(),
                            span,
                        })
                    }
                    custom => {
                        let mut children = Vec::new();
                        for arg in &call.args {
                            children.extend(self.lower_view_children(arg));
                        }
                        Some(ViewNode {
                            kind: ViewKind::Custom(custom.to_string()),
                            children,
                            modifiers: Vec::new(),
                            span,
                        })
                    }
                }
            }
            syn::Expr::MethodCall(method_call) => {
                let mut base = self.lower_view_expr(&method_call.receiver)?;
                let method_name = method_call.method.to_string();
                match method_name.as_str() {
                    "padding" => {
                        let val = method_call.args.first().and_then(|a| {
                            if let syn::Expr::Lit(syn::ExprLit {
                                lit: syn::Lit::Int(i),
                                ..
                            }) = a
                            {
                                i.base10_parse::<i32>().ok()
                            } else {
                                None
                            }
                        });
                        base.modifiers.push(ViewModifier::Padding(val));
                    }
                    "font" => {
                        let font_str = method_call
                            .args
                            .first()
                            .map(|a| self.extract_string_or_ident(a))
                            .unwrap_or_else(|| "body".into());
                        base.modifiers.push(ViewModifier::Font(font_str));
                    }
                    "foreground_color" | "foregroundColor" => {
                        let color_str = method_call
                            .args
                            .first()
                            .map(|a| self.extract_string_or_ident(a))
                            .unwrap_or_else(|| "primary".into());
                        base.modifiers
                            .push(ViewModifier::ForegroundColor(color_str));
                    }
                    "frame" => {
                        let mut width = None;
                        let mut height = None;
                        if let Some(syn::Expr::Lit(syn::ExprLit {
                            lit: syn::Lit::Int(i),
                            ..
                        })) = method_call.args.first()
                        {
                            width = i.base10_parse::<i32>().ok();
                        }
                        if let Some(syn::Expr::Lit(syn::ExprLit {
                            lit: syn::Lit::Int(i),
                            ..
                        })) = method_call.args.iter().nth(1)
                        {
                            height = i.base10_parse::<i32>().ok();
                        }
                        base.modifiers.push(ViewModifier::Frame { width, height });
                    }
                    "disabled" => {
                        let cond = method_call
                            .args
                            .first()
                            .map(|a| self.extract_string_or_ident(a))
                            .unwrap_or_default();
                        base.modifiers.push(ViewModifier::Disabled(cond));
                    }
                    "background" => {
                        let bg = method_call
                            .args
                            .first()
                            .map(|a| self.extract_string_or_ident(a))
                            .unwrap_or_default();
                        base.modifiers.push(ViewModifier::Background(bg));
                    }
                    _ => {}
                }
                Some(base)
            }
            syn::Expr::Path(p) => {
                let name = p.path.segments.last().map(|s| s.ident.to_string())?;
                match name.as_str() {
                    "Spacer" | "spacer" => Some(ViewNode {
                        kind: ViewKind::Spacer,
                        children: Vec::new(),
                        modifiers: Vec::new(),
                        span,
                    }),
                    "Divider" | "divider" => Some(ViewNode {
                        kind: ViewKind::Divider,
                        children: Vec::new(),
                        modifiers: Vec::new(),
                        span,
                    }),
                    _ => None,
                }
            }
            syn::Expr::Block(blk) => {
                for stmt in blk.block.stmts.iter().rev() {
                    if let syn::Stmt::Expr(e, _) = stmt {
                        if let Some(node) = self.lower_view_expr(e) {
                            return Some(node);
                        }
                    }
                }
                None
            }
            _ => None,
        }
    }

    fn lower_view_children(&mut self, expr: &syn::Expr) -> Vec<ViewNode> {
        match expr {
            syn::Expr::Array(arr) => arr
                .elems
                .iter()
                .filter_map(|e| self.lower_view_expr(e))
                .collect(),
            syn::Expr::Macro(m) => {
                if let Ok(elems) = m.mac.parse_body_with(
                    syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated,
                ) {
                    elems
                        .iter()
                        .filter_map(|e| self.lower_view_expr(e))
                        .collect()
                } else {
                    Vec::new()
                }
            }
            other => {
                if let Some(child) = self.lower_view_expr(other) {
                    vec![child]
                } else {
                    Vec::new()
                }
            }
        }
    }

    fn extract_string_or_ident(&self, expr: &syn::Expr) -> String {
        match expr {
            syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(s),
                ..
            }) => s.value(),
            syn::Expr::Path(p) => p
                .path
                .segments
                .iter()
                .map(|s| s.ident.to_string())
                .collect::<Vec<_>>()
                .join("."),
            syn::Expr::Field(f) => {
                let base = self.extract_string_or_ident(&f.base);
                match &f.member {
                    syn::Member::Named(id) => format!("{}.{}", base, id),
                    syn::Member::Unnamed(idx) => format!("{}.{}", base, idx.index),
                }
            }
            syn::Expr::MethodCall(m) => {
                let base = self.extract_string_or_ident(&m.receiver);
                format!("{}.{}()", base, m.method)
            }
            _ => quote::quote!(#expr).to_string(),
        }
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

        let is_codable = self.default_codable || Self::has_codable_derive(&item_struct.attrs);
        let is_observable =
            self.default_observable || Self::has_observable_attr(&item_struct.attrs);

        Some(StructDef {
            name,
            fields,
            is_public,
            is_codable,
            is_observable,
            span: self.make_span(item_struct),
        })
    }

    fn lower_enum(&mut self, item_enum: &syn::ItemEnum) -> Option<EnumDef> {
        let name = item_enum.ident.to_string();
        let is_public = matches!(item_enum.vis, syn::Visibility::Public(_));
        let mut variants = Vec::new();

        for variant in &item_enum.variants {
            let v_name = variant.ident.to_string();
            let payload = match &variant.fields {
                syn::Fields::Unit => VariantPayload::Unit,
                syn::Fields::Unnamed(unnamed) => {
                    let types = unnamed
                        .unnamed
                        .iter()
                        .map(|f| self.lower_type(&f.ty))
                        .collect();
                    VariantPayload::Tuple(types)
                }
                syn::Fields::Named(named) => {
                    let fields = named
                        .named
                        .iter()
                        .map(|f| {
                            let fname = f.ident.as_ref().map(|i| i.to_string()).unwrap_or_default();
                            let ty = self.lower_type(&f.ty);
                            let field_pub = matches!(f.vis, syn::Visibility::Public(_));
                            FieldDef {
                                name: fname,
                                ty,
                                is_public: field_pub,
                            }
                        })
                        .collect();
                    VariantPayload::Struct(fields)
                }
            };
            variants.push(EnumVariant {
                name: v_name,
                payload,
            });
        }

        let is_codable = self.default_codable || Self::has_codable_derive(&item_enum.attrs);

        Some(EnumDef {
            name,
            variants,
            is_public,
            is_codable,
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
                        "i128" | "u128" => {
                            self.reject(
                                DiagnosticCode::Int128UnsupportedRejected,
                                format!(
                                    "128-bit integer type '{}' is rejected in v0.1: Swift has no portable native 128-bit integer representation (silent narrowing to 64-bit is forbidden)",
                                    ident_str
                                ),
                                &segment.ident,
                                Some("Use i64 or u64"),
                            );
                            Type::Unit
                        }
                        "isize" => Type::Isize,
                        "u8" => Type::U8,
                        "u16" => Type::U16,
                        "u32" => Type::U32,
                        "u64" => Type::U64,
                        "usize" => Type::Usize,
                        "bool" => Type::Bool,
                        "String" | "str" => Type::String,
                        "Option" => {
                            if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                                if let Some(syn::GenericArgument::Type(inner_ty)) =
                                    args.args.first()
                                {
                                    let elem = self.lower_type(inner_ty);
                                    return Type::Option(Box::new(elem));
                                }
                            }
                            Type::Named("Option".into())
                        }
                        "Result" => {
                            if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                                let mut iter = args.args.iter().filter_map(|a| match a {
                                    syn::GenericArgument::Type(t) => Some(self.lower_type(t)),
                                    _ => None,
                                });
                                if let (Some(ok), Some(err)) = (iter.next(), iter.next()) {
                                    return Type::Result {
                                        ok: Box::new(ok),
                                        err: Box::new(err),
                                    };
                                }
                            }
                            Type::Named("Result".into())
                        }
                        "Vec" => {
                            if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                                if let Some(syn::GenericArgument::Type(inner_ty)) =
                                    args.args.first()
                                {
                                    let elem = self.lower_type(inner_ty);
                                    return Type::Array {
                                        element: Box::new(elem),
                                        size: None,
                                    };
                                }
                            }
                            Type::Named("Vec".into())
                        }
                        "RefCell" | "Cell" | "Mutex" | "RwLock" => {
                            self.reject(
                                DiagnosticCode::InteriorMutabilityRejected,
                                format!(
                                    "Interior mutability type '{}' is rejected in v0.1",
                                    segment.ident
                                ),
                                &segment.ident,
                                Some(
                                    "Use exclusive mutable references (&mut T) or value mutations",
                                ),
                            );
                            Type::Unit
                        }
                        "Rc" | "Arc" => {
                            self.reject(
                                DiagnosticCode::SharedReferenceRejected,
                                format!(
                                    "Shared reference-counted type '{}' is rejected in v0.1",
                                    segment.ident
                                ),
                                &segment.ident,
                                Some("Use single-owner value hierarchies or inout passing"),
                            );
                            Type::Unit
                        }
                        name => Type::Named(name.into()),
                    }
                } else {
                    Type::Unit
                }
            }
            syn::Type::Array(arr) => {
                let elem = self.lower_type(&arr.elem);
                let size = if let syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Int(li),
                    ..
                }) = &arr.len
                {
                    li.base10_parse::<usize>().ok()
                } else {
                    None
                };
                Type::Array {
                    element: Box::new(elem),
                    size,
                }
            }
            syn::Type::Slice(sl) => {
                let elem = self.lower_type(&sl.elem);
                Type::Array {
                    element: Box::new(elem),
                    size: None,
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
                    let (name, mutable, ty) = match &local.pat {
                        syn::Pat::Ident(id) => {
                            (id.ident.to_string(), id.mutability.is_some(), None)
                        }
                        syn::Pat::Type(pt) => {
                            let (n, m) = match &*pt.pat {
                                syn::Pat::Ident(id) => {
                                    (id.ident.to_string(), id.mutability.is_some())
                                }
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
                            let lowered_ty = self.lower_type(&pt.ty);
                            (n, m, Some(lowered_ty))
                        }
                        other => {
                            self.reject(
                                DiagnosticCode::UnsupportedTypeRejected,
                                "Complex pattern in let binding not supported",
                                other,
                                None,
                            );
                            ("val".into(), false, None)
                        }
                    };
                    let init = local.init.as_ref().map(|init| self.lower_expr(&init.expr));
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
                            syn::Expr::Binary(bin) => {
                                let maybe_int_op = match bin.op {
                                    syn::BinOp::AddAssign(_) => Some(IntegerOp::Add),
                                    syn::BinOp::SubAssign(_) => Some(IntegerOp::Sub),
                                    syn::BinOp::MulAssign(_) => Some(IntegerOp::Mul),
                                    syn::BinOp::DivAssign(_) => Some(IntegerOp::Div),
                                    syn::BinOp::RemAssign(_) => Some(IntegerOp::Rem),
                                    syn::BinOp::BitXorAssign(_) => Some(IntegerOp::BitXor),
                                    syn::BinOp::BitAndAssign(_) => Some(IntegerOp::BitAnd),
                                    syn::BinOp::BitOrAssign(_) => Some(IntegerOp::BitOr),
                                    syn::BinOp::ShlAssign(_) => Some(IntegerOp::Shl),
                                    syn::BinOp::ShrAssign(_) => Some(IntegerOp::Shr),
                                    _ => None,
                                };
                                if let Some(op) = maybe_int_op {
                                    let target = self.lower_expr(&bin.left);
                                    let value = self.lower_expr(&bin.right);
                                    let res_ty = target.ty.clone();
                                    let binary_val = Expr::new(
                                        ExprKind::Binary {
                                            op: BinaryOp::Arithmetic {
                                                op,
                                                mode: IntegerMode::Checked,
                                            },
                                            left: Box::new(target.clone()),
                                            right: Box::new(value),
                                        },
                                        res_ty,
                                    );
                                    stmts.push(Stmt::Assign {
                                        target,
                                        value: binary_val,
                                    });
                                } else {
                                    let ir_expr = self.lower_expr(expr);
                                    stmts.push(Stmt::Expr(ir_expr));
                                }
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
                    if let Some(stmt) = self.lower_macro_stmt(&m.mac) {
                        stmts.push(stmt);
                    }
                }
            }
        }

        let ty = final_expr
            .as_ref()
            .map(|e| e.ty.clone())
            .unwrap_or(Type::Unit);

        Expr::new(
            ExprKind::Block {
                stmts,
                expr: final_expr,
            },
            ty,
        )
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
                    syn::BinOp::Add(_) => BinaryOp::Arithmetic {
                        op: IntegerOp::Add,
                        mode: IntegerMode::Checked,
                    },
                    syn::BinOp::Sub(_) => BinaryOp::Arithmetic {
                        op: IntegerOp::Sub,
                        mode: IntegerMode::Checked,
                    },
                    syn::BinOp::Mul(_) => BinaryOp::Arithmetic {
                        op: IntegerOp::Mul,
                        mode: IntegerMode::Checked,
                    },
                    syn::BinOp::Div(_) => BinaryOp::Arithmetic {
                        op: IntegerOp::Div,
                        mode: IntegerMode::Checked,
                    },
                    syn::BinOp::Rem(_) => BinaryOp::Arithmetic {
                        op: IntegerOp::Rem,
                        mode: IntegerMode::Checked,
                    },
                    syn::BinOp::BitAnd(_) => BinaryOp::Arithmetic {
                        op: IntegerOp::BitAnd,
                        mode: IntegerMode::Checked,
                    },
                    syn::BinOp::BitOr(_) => BinaryOp::Arithmetic {
                        op: IntegerOp::BitOr,
                        mode: IntegerMode::Checked,
                    },
                    syn::BinOp::BitXor(_) => BinaryOp::Arithmetic {
                        op: IntegerOp::BitXor,
                        mode: IntegerMode::Checked,
                    },
                    syn::BinOp::Shl(_) => BinaryOp::Arithmetic {
                        op: IntegerOp::Shl,
                        mode: IntegerMode::Checked,
                    },
                    syn::BinOp::Shr(_) => BinaryOp::Arithmetic {
                        op: IntegerOp::Shr,
                        mode: IntegerMode::Checked,
                    },
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
                let mut mut_borrowed_vars = std::collections::HashSet::new();
                for arg in &call.args {
                    if let syn::Expr::Reference(r) = arg {
                        if r.mutability.is_some() {
                            if let syn::Expr::Path(p) = &*r.expr {
                                if let Some(ident) = p.path.get_ident() {
                                    let name = ident.to_string();
                                    if !mut_borrowed_vars.insert(name.clone()) {
                                        self.reject(
                                            DiagnosticCode::AliasedMutationRejected,
                                            format!("Simultaneous mutable borrows of '{name}' are rejected"),
                                            arg,
                                            Some("Ensure exclusive access when passing &mut parameters"),
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
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
                Expr::new(
                    ExprKind::Call {
                        func: func_name,
                        args,
                    },
                    Type::Unit,
                )
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
                    ExprKind::StructInit {
                        name: name.clone(),
                        fields,
                    },
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
            syn::Expr::ForLoop(f) => {
                let pattern = self.lower_pattern(&f.pat);
                let iter = Box::new(self.lower_expr(&f.expr));
                let body = Box::new(self.lower_block(&f.body));
                Expr::new(
                    ExprKind::For {
                        pattern,
                        iter,
                        body,
                    },
                    Type::Unit,
                )
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
            syn::Expr::Array(arr) => {
                let elems = arr.elems.iter().map(|e| self.lower_expr(e)).collect();
                Expr::new(
                    ExprKind::Array(elems),
                    Type::Array {
                        element: Box::new(Type::Unit),
                        size: None,
                    },
                )
            }
            syn::Expr::Reference(r) => {
                let inner = self.lower_expr(&r.expr);
                if r.mutability.is_some() {
                    let ty = Type::Ref {
                        inner: Box::new(inner.ty.clone()),
                        mutable: true,
                    };
                    Expr::new(
                        ExprKind::Borrow {
                            expr: Box::new(inner),
                            mutable: true,
                        },
                        ty,
                    )
                } else {
                    inner
                }
            }
            syn::Expr::Index(idx) => {
                let expr = Box::new(self.lower_expr(&idx.expr));
                match &*idx.index {
                    syn::Expr::Range(r) => {
                        let start = r.start.as_ref().map(|s| Box::new(self.lower_expr(s)));
                        let end = r.end.as_ref().map(|e| Box::new(self.lower_expr(e)));
                        Expr::new(ExprKind::Slice { expr, start, end }, Type::String)
                    }
                    _ => {
                        let index = Box::new(self.lower_expr(&idx.index));
                        Expr::new(ExprKind::Index { expr, index }, Type::Unit)
                    }
                }
            }
            syn::Expr::Cast(c) => {
                let expr = self.lower_expr(&c.expr);
                let target_ty = self.lower_type(&c.ty);
                let func_name = match &target_ty {
                    Type::I8 => "Int8",
                    Type::I16 => "Int16",
                    Type::I32 => "Int32",
                    Type::I64 => "Int64",
                    Type::I128 => "Int64",
                    Type::Isize => "Int",
                    Type::U8 => "UInt8",
                    Type::U16 => "UInt16",
                    Type::U32 => "UInt32",
                    Type::U64 => "UInt64",
                    Type::U128 => "UInt64",
                    Type::Usize => "UInt",
                    _ => return expr,
                };
                Expr::new(
                    ExprKind::Call {
                        func: func_name.into(),
                        args: vec![expr],
                    },
                    target_ty,
                )
            }
            syn::Expr::Macro(m) => self.lower_macro_expr(&m.mac),
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

    fn extract_macro_str_literal(expr: &syn::Expr) -> Option<String> {
        if let syn::Expr::Lit(syn::ExprLit {
            lit: syn::Lit::Str(s),
            ..
        }) = expr
        {
            Some(s.value())
        } else {
            None
        }
    }

    fn lower_macro_stmt(&mut self, mac: &syn::Macro) -> Option<Stmt> {
        use syn::parse::Parser;
        use syn::punctuated::Punctuated;
        use syn::Token;

        let macro_name = mac
            .path
            .segments
            .last()
            .map(|s| s.ident.to_string())
            .unwrap_or_default();

        match macro_name.as_str() {
            "assert" => {
                let parser = Punctuated::<syn::Expr, Token![,]>::parse_terminated;
                match parser.parse2(mac.tokens.clone()) {
                    Ok(args) if !args.is_empty() => {
                        let condition = self.lower_expr(&args[0]);
                        let message = if args.len() > 1 {
                            Self::extract_macro_str_literal(&args[1])
                        } else {
                            None
                        };
                        Some(Stmt::Assert { condition, message })
                    }
                    _ => {
                        self.reject(
                            DiagnosticCode::SyntaxError,
                            "Invalid arguments to assert! macro",
                            mac,
                            Some("assert!(condition, [message])"),
                        );
                        None
                    }
                }
            }
            "debug_assert" => {
                let parser = Punctuated::<syn::Expr, Token![,]>::parse_terminated;
                match parser.parse2(mac.tokens.clone()) {
                    Ok(args) if !args.is_empty() => {
                        let condition = self.lower_expr(&args[0]);
                        let message = if args.len() > 1 {
                            Self::extract_macro_str_literal(&args[1])
                        } else {
                            None
                        };
                        Some(Stmt::DebugAssert { condition, message })
                    }
                    _ => {
                        self.reject(
                            DiagnosticCode::SyntaxError,
                            "Invalid arguments to debug_assert! macro",
                            mac,
                            Some("debug_assert!(condition, [message])"),
                        );
                        None
                    }
                }
            }
            "assert_eq" => {
                let parser = Punctuated::<syn::Expr, Token![,]>::parse_terminated;
                match parser.parse2(mac.tokens.clone()) {
                    Ok(args) if args.len() >= 2 => {
                        let left = self.lower_expr(&args[0]);
                        let right = self.lower_expr(&args[1]);
                        let message = if args.len() > 2 {
                            Self::extract_macro_str_literal(&args[2])
                        } else {
                            None
                        };
                        Some(Stmt::AssertEq {
                            left,
                            right,
                            message,
                        })
                    }
                    _ => {
                        self.reject(
                            DiagnosticCode::SyntaxError,
                            "assert_eq! expects at least two arguments",
                            mac,
                            Some("assert_eq!(left, right, [message])"),
                        );
                        None
                    }
                }
            }
            "panic" => {
                let parser = Punctuated::<syn::Expr, Token![,]>::parse_terminated;
                let message = match parser.parse2(mac.tokens.clone()) {
                    Ok(args) if !args.is_empty() => Self::extract_macro_str_literal(&args[0]),
                    _ => None,
                };
                Some(Stmt::Panic { message })
            }
            _ => {
                self.reject(
                    DiagnosticCode::MacroRejected,
                    format!("Macro '{macro_name}!' is rejected in v0.1"),
                    mac,
                    Some("Only assert!, debug_assert!, assert_eq!, and panic! are admitted"),
                );
                None
            }
        }
    }

    fn lower_macro_expr(&mut self, mac: &syn::Macro) -> Expr {
        use syn::parse::Parser;
        use syn::punctuated::Punctuated;
        use syn::Token;

        let macro_name = mac
            .path
            .segments
            .last()
            .map(|s| s.ident.to_string())
            .unwrap_or_default();

        match macro_name.as_str() {
            "panic" => {
                let parser = Punctuated::<syn::Expr, Token![,]>::parse_terminated;
                let message = match parser.parse2(mac.tokens.clone()) {
                    Ok(args) if !args.is_empty() => Self::extract_macro_str_literal(&args[0]),
                    _ => None,
                };
                Expr::new(ExprKind::Panic(message), Type::Unit)
            }
            "vec" => {
                let parser = Punctuated::<syn::Expr, Token![,]>::parse_terminated;
                let elems = match parser.parse2(mac.tokens.clone()) {
                    Ok(args) => args.iter().map(|a| self.lower_expr(a)).collect(),
                    _ => Vec::new(),
                };
                Expr::new(
                    ExprKind::Array(elems),
                    Type::Array {
                        element: Box::new(Type::Unit),
                        size: None,
                    },
                )
            }
            _ => {
                self.reject(
                    DiagnosticCode::MacroRejected,
                    format!("Macro '{macro_name}!' in expression position is rejected in v0.1"),
                    mac,
                    Some("Only panic! and vec! are admitted in expression position"),
                );
                Expr::new(ExprKind::Literal(Literal::Unit), Type::Unit)
            }
        }
    }

    fn lower_pattern(&mut self, pat: &syn::Pat) -> Pattern {
        match pat {
            syn::Pat::Wild(_) => Pattern::Wildcard,
            syn::Pat::Ident(id) => {
                let name = id.ident.to_string();
                if name == "None" {
                    Pattern::EnumVariant {
                        enum_name: None,
                        variant: "None".into(),
                    }
                } else {
                    Pattern::Variable(name)
                }
            }
            syn::Pat::Lit(lit) => match &lit.lit {
                syn::Lit::Int(i) => {
                    let val: i128 = i.base10_parse().unwrap_or(0);
                    Pattern::Literal(Literal::Int {
                        value: val,
                        ty: Type::I32,
                    })
                }
                syn::Lit::Bool(b) => Pattern::Literal(Literal::Bool(b.value)),
                syn::Lit::Str(s) => Pattern::Literal(Literal::String(s.value())),
                _ => Pattern::Wildcard,
            },
            syn::Pat::Path(p) => {
                let segments: Vec<String> = p
                    .path
                    .segments
                    .iter()
                    .map(|s| s.ident.to_string())
                    .collect();
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
            syn::Pat::TupleStruct(ts) => {
                let segments: Vec<String> = ts
                    .path
                    .segments
                    .iter()
                    .map(|s| s.ident.to_string())
                    .collect();
                let (enum_name, variant) = if segments.len() >= 2 {
                    (
                        Some(segments[..segments.len() - 1].join(".")),
                        segments.last().unwrap().clone(),
                    )
                } else if let Some(last) = segments.last() {
                    (None, last.clone())
                } else {
                    (None, "Unknown".into())
                };

                let patterns = ts.elems.iter().map(|p| self.lower_pattern(p)).collect();
                Pattern::EnumVariantPayload {
                    enum_name,
                    variant,
                    patterns,
                }
            }
            syn::Pat::Paren(p) => self.lower_pattern(&p.pat),
            syn::Pat::Reference(r) => self.lower_pattern(&r.pat),
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
                    "i128" | "u128" => {
                        self.reject(
                            DiagnosticCode::Int128UnsupportedRejected,
                            "128-bit integer literals are rejected in v0.1: Swift has no portable native 128-bit integer representation (silent narrowing to 64-bit is forbidden)",
                            i,
                            Some("Use i64 or u64"),
                        );
                        Type::I64
                    }
                    "isize" => Type::Isize,
                    "u8" => Type::U8,
                    "u16" => Type::U16,
                    "u32" => Type::U32,
                    "u64" => Type::U64,
                    "usize" => Type::Usize,
                    _ => Type::I32,
                };
                if ty.is_signed_integer() {
                    let val: i128 = i.base10_parse().unwrap_or(0);
                    Expr::new(
                        ExprKind::Literal(Literal::Int {
                            value: val,
                            ty: ty.clone(),
                        }),
                        ty,
                    )
                } else {
                    let val: u128 = i.base10_parse().unwrap_or(0);
                    Expr::new(
                        ExprKind::Literal(Literal::UInt {
                            value: val,
                            ty: ty.clone(),
                        }),
                        ty,
                    )
                }
            }
            syn::Lit::Bool(b) => Expr::new(ExprKind::Literal(Literal::Bool(b.value)), Type::Bool),
            syn::Lit::Str(s) => {
                Expr::new(ExprKind::Literal(Literal::String(s.value())), Type::String)
            }
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
        let errs = frontend
            .parse_and_lower(src)
            .expect_err("Should reject unsafe");
        assert!(errs
            .iter()
            .any(|d| d.code == DiagnosticCode::UnsafeRejected));
    }

    #[test]
    fn test_async_is_rejected_with_stable_diagnostic() {
        let src = r#"
            pub async fn fetch() -> i32 {
                42
            }
        "#;
        let mut frontend = Frontend::new("async.rs");
        let errs = frontend
            .parse_and_lower(src)
            .expect_err("Should reject async");
        assert!(errs.iter().any(|d| d.code == DiagnosticCode::AsyncRejected));
    }

    #[test]
    fn test_admitted_contract_macros_lower() {
        let src = r#"
            pub fn checked_step(x: i32) -> i32 {
                assert!(x > 0);
                debug_assert!(x < 100, "within range");
                assert_eq!(x, 42, "magic value");
                if x == 99 {
                    panic!("illegal boundary");
                }
                x
            }
        "#;
        let mut frontend = Frontend::new("contract.rs");
        let module = frontend
            .parse_and_lower(src)
            .expect("Should parse contract macros");
        assert_eq!(module.items.len(), 1);
        if let IrItem::Function(func) = &module.items[0] {
            if let ExprKind::Block { stmts, .. } = &func.body.kind {
                assert!(stmts.iter().any(|s| matches!(s, Stmt::Assert { .. })));
                assert!(stmts.iter().any(|s| matches!(s, Stmt::DebugAssert { .. })));
                assert!(stmts.iter().any(|s| matches!(s, Stmt::AssertEq { .. })));
            } else {
                panic!("Expected block body");
            }
        } else {
            panic!("Expected function");
        }
    }

    #[test]
    fn test_unadmitted_macro_rejected() {
        let src = r#"
            pub fn log() {
                println!("unsupported macro in v0.1");
            }
        "#;
        let mut frontend = Frontend::new("macro_reject.rs");
        let errs = frontend
            .parse_and_lower(src)
            .expect_err("Should reject println!");
        assert!(errs.iter().any(|d| d.code == DiagnosticCode::MacroRejected));
    }

    #[test]
    fn test_impl_methods_and_payload_enums_parse_and_lower() {
        let src = r#"
            pub struct Counter {
                pub count: i64,
            }

            impl Counter {
                pub fn new(initial: i64) -> Counter {
                    Counter { count: initial }
                }

                pub fn increment(&mut self, delta: i64) {
                    self.count = self.count + delta;
                }

                pub fn get(&self) -> i64 {
                    self.count
                }
            }

            pub enum Action {
                Stop,
                Move(i64, i64),
                Teleport { x: i64, y: i64 },
            }

            pub fn apply(action: Action) -> (i64, i64) {
                match action {
                    Action::Stop => (0, 0),
                    Action::Move(dx, dy) => (dx, dy),
                    Action::Teleport { x, y } => (x, y),
                }
            }
        "#;
        let mut frontend = Frontend::new("types.rs");
        let module = frontend.parse_and_lower(src).expect("Should succeed");
        assert_eq!(module.items.len(), 4);
    }

    #[test]
    fn test_complex_trait_rejected_with_e0006() {
        let src = r#"
            pub trait ComplexTrait {
                type Item;
            }
        "#;
        let mut frontend = Frontend::new("complex_trait.rs");
        let errs = frontend
            .parse_and_lower(src)
            .expect_err("Should reject complex trait with associated types");
        assert!(errs
            .iter()
            .any(|d| d.code == DiagnosticCode::ComplexTraitRejected));
    }

    #[test]
    fn test_trait_and_trait_impl_lowering() {
        let src = r#"
            pub trait Greeter {
                fn greet(&self) -> String;
                fn tag(&self) -> i32 {
                    42
                }
            }

            pub struct Person {
                pub name: String,
            }

            impl Greeter for Person {
                fn greet(&self) -> String {
                    self.name
                }
            }
        "#;
        let mut frontend = Frontend::new("greeter.rs");
        let module = frontend
            .parse_and_lower(src)
            .expect("Should parse and lower valid trait and trait impl");
        assert_eq!(module.items.len(), 3);
        match &module.items[0] {
            IrItem::Trait(trt) => {
                assert_eq!(trt.name, "Greeter");
                assert_eq!(trt.methods.len(), 2);
                assert!(trt.methods[1].default_body.is_some());
            }
            _ => panic!("Expected IrItem::Trait"),
        }
        match &module.items[2] {
            IrItem::TraitImpl(timpl) => {
                assert_eq!(timpl.trait_name, "Greeter");
                assert_eq!(timpl.target_type, "Person");
                assert_eq!(timpl.methods.len(), 1);
            }
            _ => panic!("Expected IrItem::TraitImpl"),
        }
    }

    #[test]
    fn test_dyn_trait_rejected_with_e0005() {
        let src = r#"
            pub fn call_dyn(x: &dyn Display) {}
        "#;
        let mut frontend = Frontend::new("dyn_trait.rs");
        let errs = frontend
            .parse_and_lower(src)
            .expect_err("Should reject dynamic dispatch");
        assert!(errs
            .iter()
            .any(|d| d.code == DiagnosticCode::DynamicDispatchRejected));
    }

    #[test]
    fn test_generics_rejected_with_e0011() {
        let src = r#"
            pub fn identity<T>(x: T) -> T {
                x
            }
        "#;
        let mut frontend = Frontend::new("generic.rs");
        let errs = frontend
            .parse_and_lower(src)
            .expect_err("Should reject unspecialized generic function");
        assert!(errs
            .iter()
            .any(|d| d.code == DiagnosticCode::UnspecializableGenericRejected));
    }

    #[test]
    fn test_option_result_string_parse_and_lower() {
        let src = r#"
            pub fn process_data(opt: Option<i64>, res: Result<i64, i32>, s: String) -> i64 {
                let is_s = opt.is_some();
                let sub = &s[0..5];
                match res {
                    Ok(val) => val,
                    Err(e) => 0,
                }
            }
        "#;
        let mut frontend = Frontend::new("data.rs");
        let module = frontend
            .parse_and_lower(src)
            .expect("Should parse and lower data semantics");
        assert_eq!(module.items.len(), 1);
    }

    #[test]
    fn test_custom_drop_rejected_with_e0007() {
        let src = r#"
            pub struct Resource;
            impl Drop for Resource {
                fn drop(&mut self) {}
            }
        "#;
        let mut frontend = Frontend::new("drop.rs");
        let errs = frontend
            .parse_and_lower(src)
            .expect_err("Should reject custom Drop");
        assert!(errs
            .iter()
            .any(|d| d.code == DiagnosticCode::CustomDropRejected));
    }

    #[test]
    fn test_interior_mutability_rejected_with_e0012() {
        let src = r#"
            pub fn use_cell(c: RefCell<i32>) {}
        "#;
        let mut frontend = Frontend::new("cell.rs");
        let errs = frontend
            .parse_and_lower(src)
            .expect_err("Should reject interior mutability");
        assert!(errs
            .iter()
            .any(|d| d.code == DiagnosticCode::InteriorMutabilityRejected));
    }

    #[test]
    fn test_aliased_mutation_rejected_with_e0006() {
        let src = r#"
            pub fn mutate_two(a: &mut i32, b: &mut i32) {}
            pub fn test_alias() {
                let mut x = 10;
                mutate_two(&mut x, &mut x);
            }
        "#;
        let mut frontend = Frontend::new("alias.rs");
        let errs = frontend
            .parse_and_lower(src)
            .expect_err("Should reject aliased mutation");
        assert!(errs
            .iter()
            .any(|d| d.code == DiagnosticCode::AliasedMutationRejected));
    }

    #[test]
    fn test_inout_parameter_and_borrow_lowering() {
        let src = r#"
            pub fn add_amount(val: &mut i64, delta: i64) {
                *val += delta;
            }
        "#;
        let mut frontend = Frontend::new("inout.rs");
        let module = frontend
            .parse_and_lower(src)
            .expect("Should parse and lower inout function");
        assert_eq!(module.items.len(), 1);
    }

    #[test]
    fn test_for_loop_and_vec_lowering() {
        let src = r#"
            pub fn sum_elements(items: Vec<i32>) -> i32 {
                let mut total = 0;
                for x in items {
                    total += x;
                }
                total
            }
        "#;
        let mut frontend = Frontend::new("loops.rs");
        let module = frontend
            .parse_and_lower(src)
            .expect("Should parse and lower for loop");
        assert_eq!(module.items.len(), 1);
        if let IrItem::Function(func) = &module.items[0] {
            if let ExprKind::Block { stmts, .. } = &func.body.kind {
                assert!(stmts.iter().any(|s| matches!(
                    s,
                    Stmt::Expr(Expr {
                        kind: ExprKind::For { .. },
                        ..
                    })
                )));
            } else {
                panic!("Expected function body to be a block");
            }
        } else {
            panic!("Expected Item::Function");
        }
    }

    #[test]
    fn test_codable_derive_lowering() {
        let src = r#"
            #[derive(Serialize, Deserialize)]
            pub struct UserProfile {
                pub id: u64,
                pub username: String,
            }

            #[derive(Serialize, Deserialize)]
            pub enum AccessLevel {
                Admin,
                Guest,
                Custom(String),
            }
        "#;
        let mut frontend = Frontend::new("codable_test.rs");
        let module = frontend
            .parse_and_lower(src)
            .expect("Should parse and lower struct and enum with serde derives");

        assert_eq!(module.items.len(), 2);
        if let IrItem::Struct(s) = &module.items[0] {
            assert_eq!(s.name, "UserProfile");
            assert!(s.is_codable);
        } else {
            panic!("Expected struct item");
        }

        if let IrItem::Enum(e) = &module.items[1] {
            assert_eq!(e.name, "AccessLevel");
            assert!(e.is_codable);
        } else {
            panic!("Expected enum item");
        }
    }

    #[test]
    fn test_view_component_lowering() {
        let src = r#"
            #[observable]
            pub struct TaskState {
                pub title: String,
                pub is_done: bool,
            }

            #[view]
            pub struct TaskView {
                pub state: TaskState,
            }

            impl TaskView {
                pub fn body(&self) {
                    VStack(vec![
                        Text("Task Details").font("headline").padding(),
                        HStack(vec![
                            Text("Done"),
                            Toggle("Status", "$state.is_done"),
                            Spacer(),
                            Button("Delete", "state.delete()"),
                        ]),
                    ])
                }
            }
        "#;

        let mut frontend = Frontend::new("ui_test.rs");
        let module = frontend
            .parse_and_lower(src)
            .expect("Should parse and lower TaskState and TaskView");

        assert_eq!(module.items.len(), 2);
        assert!(
            matches!(&module.items[0], IrItem::Struct(s) if s.name == "TaskState" && s.is_observable)
        );
        if let IrItem::View(v) = &module.items[1] {
            assert_eq!(v.name, "TaskView");
            assert_eq!(v.properties.len(), 1);
            assert_eq!(v.properties[0].name, "state");
            assert_eq!(v.properties[0].kind, PropertyBindingKind::Observable);
            assert!(matches!(v.root.kind, ViewKind::VStack { .. }));
            assert_eq!(v.root.children.len(), 2);
            assert!(matches!(v.root.children[0].kind, ViewKind::Text(_)));
            assert_eq!(v.root.children[0].modifiers.len(), 2);
            assert!(matches!(v.root.children[1].kind, ViewKind::HStack { .. }));
            assert_eq!(v.root.children[1].children.len(), 4);
        } else {
            panic!("Expected Item::View");
        }
    }
}
