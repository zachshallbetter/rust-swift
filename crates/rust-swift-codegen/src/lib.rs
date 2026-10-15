//! Swift backend code generator and standalone package emitter.
//!
//! Emits clean, deterministic, independently compilable Swift code from the
//! Semantic IR. Supports native Swift overflow operators (`&+`, `&-`, `&*`)
//! for exact compatibility with Rust wrapping operations.

use rust_swift_ir::{
    BinaryOp, CmpOp, ConstantDef, EnumDef, Expr, ExprKind, Function, ImplBlock, IntegerMode,
    IntegerOp, Item, Literal, LogicalOp, MatchArm, Method, Module, Pattern, PropertyBindingKind,
    ReceiverKind, Stmt, StructDef, TraitDef, TraitImpl, TraitMethod, Type, TypeAlias, UnaryOp,
    VariantPayload, ViewDef, ViewKind, ViewModifier, ViewNode,
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolMapping {
    pub rust_name: String,
    pub swift_name: String,
    pub kind: String,
    pub rust_file: String,
    pub rust_line: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageManifest {
    pub package_name: String,
    pub swift_tools_version: String,
    #[serde(default)]
    pub platforms: Vec<String>,
    pub generated_files: Vec<String>,
    pub symbols: Vec<SymbolMapping>,
    #[serde(default)]
    pub source_locations_enabled: bool,
    #[serde(default)]
    pub test_target_generated: bool,
}

pub struct SwiftGenerator {
    indent_level: usize,
    symbols: Vec<SymbolMapping>,
    fn_inouts: std::collections::HashMap<String, Vec<bool>>,
    emit_source_locations: bool,
    emit_observable: bool,
    observable_types: std::collections::HashSet<String>,
}

impl Default for SwiftGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl SwiftGenerator {
    pub fn new() -> Self {
        Self {
            indent_level: 0,
            symbols: Vec::new(),
            fn_inouts: std::collections::HashMap::new(),
            emit_source_locations: false,
            emit_observable: false,
            observable_types: std::collections::HashSet::new(),
        }
    }

    pub fn with_source_locations(mut self, enabled: bool) -> Self {
        self.emit_source_locations = enabled;
        self
    }

    pub fn set_source_locations(&mut self, enabled: bool) {
        self.emit_source_locations = enabled;
    }

    pub fn with_observable(mut self, enabled: bool) -> Self {
        self.emit_observable = enabled;
        self
    }

    pub fn set_observable(&mut self, enabled: bool) {
        self.emit_observable = enabled;
    }

    fn indent(&self) -> String {
        "    ".repeat(self.indent_level)
    }

    fn emit_doc_comments(&self, out: &mut String, doc_comments: &[String]) {
        for doc in doc_comments {
            let trimmed = doc.trim_end();
            if trimmed.is_empty() {
                out.push_str(&format!("{}///\n", self.indent()));
            } else if trimmed.starts_with(' ') {
                out.push_str(&format!("{}///{}\n", self.indent(), trimmed));
            } else {
                out.push_str(&format!("{}/// {}\n", self.indent(), trimmed));
            }
        }
    }

    pub fn generate_module(&mut self, module: &Module) -> (String, Vec<SymbolMapping>) {
        // Index function and method parameter inout mutabilities
        for item in &module.items {
            match item {
                Item::Function(func) => {
                    let inouts = func.params.iter().map(|p| p.is_exclusive_ref()).collect();
                    self.fn_inouts.insert(func.name.clone(), inouts);
                }
                Item::Impl(imp) => {
                    for method in &imp.methods {
                        let inouts = method.params.iter().map(|p| p.is_exclusive_ref()).collect();
                        self.fn_inouts.insert(method.name.clone(), inouts);
                    }
                }
                Item::Trait(trt) => {
                    for method in &trt.methods {
                        let inouts = method.params.iter().map(|p| p.is_exclusive_ref()).collect();
                        self.fn_inouts.insert(method.name.clone(), inouts);
                    }
                }
                Item::TraitImpl(timpl) => {
                    for method in &timpl.methods {
                        let inouts = method.params.iter().map(|p| p.is_exclusive_ref()).collect();
                        self.fn_inouts.insert(method.name.clone(), inouts);
                    }
                }
                _ => {}
            }
        }

        self.observable_types.clear();
        for item in &module.items {
            if let Item::Struct(strct) = item {
                if strct.is_observable || self.emit_observable {
                    self.observable_types.insert(strct.name.clone());
                }
            }
        }
        let has_view = module.items.iter().any(|i| matches!(i, Item::View(_)));
        let has_document = module
            .items
            .iter()
            .any(|i| matches!(i, Item::Struct(s) if s.is_document));
        let has_reality = module
            .items
            .iter()
            .any(|i| matches!(i, Item::Struct(s) if s.is_reality_component));
        let has_fskit = module
            .items
            .iter()
            .any(|i| matches!(i, Item::Struct(s) if s.is_fskit_volume));
        let has_widget = module
            .items
            .iter()
            .any(|i| matches!(i, Item::Struct(s) if s.is_widget_timeline));
        let has_observable = !self.observable_types.is_empty() || has_view;

        let mut out = String::new();
        out.push_str("//\n");
        out.push_str(&format!(
            "// Generated by rust-swift compiler from {}\n",
            module.name
        ));
        out.push_str("// Strict semantic preservation - do not edit directly.\n");
        out.push_str("//\n\n");
        out.push_str("import Foundation\n");
        if has_view || has_document {
            out.push_str("import SwiftUI\n");
        }
        if has_document {
            out.push_str("import UniformTypeIdentifiers\n");
        }
        if has_reality {
            out.push_str("import RealityKit\n");
        }
        if has_fskit {
            out.push_str("import FSKit\n");
        }
        if has_widget {
            out.push_str("import WidgetKit\n");
        }
        if has_observable {
            out.push_str("import Observation\n");
        }
        out.push('\n');

        for item in &module.items {
            match item {
                Item::Enum(enm) => {
                    out.push_str(&self.generate_enum(enm));
                    out.push_str("\n\n");
                }
                Item::Struct(strct) => {
                    out.push_str(&self.generate_struct(strct));
                    out.push_str("\n\n");
                }
                Item::View(view) => {
                    out.push_str(&self.generate_view(view));
                    out.push_str("\n\n");
                }
                Item::Impl(imp) => {
                    out.push_str(&self.generate_impl(imp));
                    out.push_str("\n\n");
                }
                Item::Trait(trt) => {
                    out.push_str(&self.generate_trait(trt));
                    out.push_str("\n\n");
                }
                Item::TraitImpl(timpl) => {
                    out.push_str(&self.generate_trait_impl(timpl));
                    out.push_str("\n\n");
                }
                Item::Function(func) => {
                    out.push_str(&self.generate_function(func));
                    out.push_str("\n\n");
                }
                Item::Constant(c) => {
                    out.push_str(&self.generate_constant(c));
                    out.push_str("\n\n");
                }
                Item::TypeAlias(ta) => {
                    out.push_str(&self.generate_type_alias(ta));
                    out.push_str("\n\n");
                }
            }
        }

        if self.emit_source_locations {
            out.push_str("#sourceLocation()\n");
        }

        (out, self.symbols.clone())
    }

    fn map_type(&self, ty: &Type) -> String {
        match ty {
            Type::I8 => "Int8".into(),
            Type::I16 => "Int16".into(),
            Type::I32 => "Int32".into(),
            Type::I64 => "Int64".into(),
            Type::I128 => {
                panic!("Type::I128 must be rejected by frontend; silent narrowing to 64-bit is forbidden")
            }
            Type::Isize => "Int".into(),
            Type::U8 => "UInt8".into(),
            Type::U16 => "UInt16".into(),
            Type::U32 => "UInt32".into(),
            Type::U64 => "UInt64".into(),
            Type::U128 => {
                panic!("Type::U128 must be rejected by frontend; silent narrowing to 64-bit is forbidden")
            }
            Type::Usize => "UInt".into(),
            Type::Bool => "Bool".into(),
            Type::Unit => "Void".into(),
            Type::String => "String".into(),
            Type::Option(inner) => format!("{}?", self.map_type(inner)),
            Type::Result { ok, err } => {
                format!("Result<{}, {}>", self.map_type(ok), self.map_type(err))
            }
            Type::Named(name) => name.clone(),
            Type::Tuple(elems) => {
                let inner = elems
                    .iter()
                    .map(|e| self.map_type(e))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("({})", inner)
            }
            Type::Array { element, .. } => {
                format!("[{}]", self.map_type(element))
            }
            Type::Ref { inner, mutable } => {
                if *mutable {
                    format!("inout {}", self.map_type(inner))
                } else {
                    self.map_type(inner)
                }
            }
        }
    }

    fn generate_enum(&mut self, enm: &EnumDef) -> String {
        self.symbols.push(SymbolMapping {
            rust_name: enm.name.clone(),
            swift_name: enm.name.clone(),
            kind: "enum".into(),
            rust_file: enm.span.file.clone(),
            rust_line: enm.span.start_line,
        });

        let mut out = String::new();
        if self.emit_source_locations && enm.span.start_line > 0 && !enm.span.file.is_empty() {
            let escaped = enm.span.file.replace('\\', "\\\\").replace('"', "\\\"");
            out.push_str(&format!(
                "{}#sourceLocation(file: \"{}\", line: {})\n",
                self.indent(),
                escaped,
                enm.span.start_line
            ));
        }
        self.emit_doc_comments(&mut out, &enm.doc_comments);
        let vis = if enm.is_public { "public " } else { "" };

        if enm.is_fieldless() {
            let conformances = if enm.is_codable {
                "String, CaseIterable, Equatable, Hashable, Codable, Sendable, Error"
            } else {
                "String, CaseIterable, Equatable, Hashable, Sendable, Error"
            };
            out.push_str(&format!("{}enum {}: {} {{\n", vis, enm.name, conformances));
            self.indent_level += 1;
            for variant in &enm.variants {
                self.emit_doc_comments(&mut out, &variant.doc_comments);
                out.push_str(&format!(
                    "{}case {} = \"{}\"\n",
                    self.indent(),
                    variant.name,
                    variant.name
                ));
            }
            self.indent_level -= 1;
            out.push_str(&format!("{}}}", self.indent()));
        } else {
            let conformances = if enm.is_codable {
                "Equatable, Hashable, Codable, Sendable, Error"
            } else {
                "Equatable, Hashable, Sendable, Error"
            };
            out.push_str(&format!("{}enum {}: {} {{\n", vis, enm.name, conformances));
            self.indent_level += 1;
            for variant in &enm.variants {
                self.emit_doc_comments(&mut out, &variant.doc_comments);
                match &variant.payload {
                    VariantPayload::Unit => {
                        out.push_str(&format!("{}case {}\n", self.indent(), variant.name));
                    }
                    VariantPayload::Tuple(types) => {
                        let types_str = types
                            .iter()
                            .map(|t| self.map_type(t))
                            .collect::<Vec<_>>()
                            .join(", ");
                        out.push_str(&format!(
                            "{}case {}({})\n",
                            self.indent(),
                            variant.name,
                            types_str
                        ));
                    }
                    VariantPayload::Struct(fields) => {
                        let fields_str = fields
                            .iter()
                            .map(|f| format!("{}: {}", f.name, self.map_type(&f.ty)))
                            .collect::<Vec<_>>()
                            .join(", ");
                        out.push_str(&format!(
                            "{}case {}({})\n",
                            self.indent(),
                            variant.name,
                            fields_str
                        ));
                    }
                }
            }
            self.indent_level -= 1;
            out.push_str(&format!("{}}}", self.indent()));
        }
        out
    }

    fn generate_struct(&mut self, strct: &StructDef) -> String {
        self.symbols.push(SymbolMapping {
            rust_name: strct.name.clone(),
            swift_name: strct.name.clone(),
            kind: "struct".into(),
            rust_file: strct.span.file.clone(),
            rust_line: strct.span.start_line,
        });

        let mut out = String::new();
        if self.emit_source_locations && strct.span.start_line > 0 && !strct.span.file.is_empty() {
            let escaped = strct.span.file.replace('\\', "\\\\").replace('"', "\\\"");
            out.push_str(&format!(
                "{}#sourceLocation(file: \"{}\", line: {})\n",
                self.indent(),
                escaped,
                strct.span.start_line
            ));
        }
        self.emit_doc_comments(&mut out, &strct.doc_comments);
        let vis = if strct.is_public { "public " } else { "" };
        let is_observable = strct.is_observable || self.emit_observable;

        if strct.is_bitflags {
            let raw_type = strct
                .fields
                .iter()
                .find(|f| f.name == "raw_value" || f.name == "rawValue" || f.name == "bits")
                .map(|f| self.map_type(&f.ty))
                .or_else(|| strct.fields.first().map(|f| self.map_type(&f.ty)))
                .unwrap_or_else(|| "UInt64".to_string());

            let conformances = if strct.is_codable {
                "OptionSet, Codable, Sendable"
            } else {
                "OptionSet, Sendable"
            };

            out.push_str(&format!(
                "{}{}struct {}: {} {{\n",
                self.indent(),
                vis,
                strct.name,
                conformances
            ));
            self.indent_level += 1;
            out.push_str(&format!(
                "{}{}let rawValue: {}\n\n",
                self.indent(),
                vis,
                raw_type
            ));
            out.push_str(&format!(
                "{}{}init(rawValue: {}) {{\n",
                self.indent(),
                vis,
                raw_type
            ));
            self.indent_level += 1;
            out.push_str(&format!("{}self.rawValue = rawValue\n", self.indent()));
            self.indent_level -= 1;
            out.push_str(&format!("{}}}\n\n", self.indent()));

            out.push_str(&format!(
                "{}{}static func | (lhs: {}, rhs: {}) -> {} {{\n",
                self.indent(),
                vis,
                strct.name,
                strct.name,
                strct.name
            ));
            self.indent_level += 1;
            out.push_str(&format!("{}lhs.union(rhs)\n", self.indent()));
            self.indent_level -= 1;
            out.push_str(&format!("{}}}\n\n", self.indent()));

            out.push_str(&format!(
                "{}{}static func & (lhs: {}, rhs: {}) -> {} {{\n",
                self.indent(),
                vis,
                strct.name,
                strct.name,
                strct.name
            ));
            self.indent_level += 1;
            out.push_str(&format!("{}lhs.intersection(rhs)\n", self.indent()));
            self.indent_level -= 1;
            out.push_str(&format!("{}}}\n", self.indent()));

            self.indent_level -= 1;
            out.push_str(&format!("{}}}\n", self.indent()));
            return out;
        }

        if is_observable {
            let has_id = strct.fields.iter().any(|f| f.name == "id");
            let conformances = if has_id { ": Identifiable" } else { "" };
            out.push_str(&format!(
                "{}@Observable\n{}{}final class {}{} {{\n",
                self.indent(),
                self.indent(),
                vis,
                strct.name,
                conformances
            ));
        } else {
            let mut confs = Vec::new();
            if strct.is_codable {
                confs.push("Equatable");
                confs.push("Hashable");
                confs.push("Codable");
                confs.push("Sendable");
            } else {
                confs.push("Equatable");
                confs.push("Hashable");
                confs.push("Sendable");
            }
            if strct.is_reality_component {
                confs.push("RealityKit.Component");
            }
            let conformances = confs.join(", ");
            out.push_str(&format!(
                "{}struct {}: {} {{\n",
                vis, strct.name, conformances
            ));
        }
        self.indent_level += 1;

        for field in &strct.fields {
            self.emit_doc_comments(&mut out, &field.doc_comments);
            let f_vis = if field.is_public { "public " } else { "" };
            out.push_str(&format!(
                "{}{}var {}: {}\n",
                self.indent(),
                f_vis,
                field.name,
                self.map_type(&field.ty)
            ));
        }

        // Explicit initializer
        out.push_str(&format!("\n{}{}init(", self.indent(), vis));
        let params_str = strct
            .fields
            .iter()
            .map(|f| format!("{}: {}", f.name, self.map_type(&f.ty)))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&params_str);
        out.push_str(") {\n");

        self.indent_level += 1;
        for field in &strct.fields {
            out.push_str(&format!(
                "{}self.{} = {}\n",
                self.indent(),
                field.name,
                field.name
            ));
        }
        self.indent_level -= 1;
        out.push_str(&format!("{}}}\n", self.indent()));

        self.indent_level -= 1;
        out.push_str(&format!("{}}}", self.indent()));

        if strct.is_document {
            out.push_str("\n\n");
            out.push_str(&format!(
                "{}extension {}: FileDocument {{\n",
                self.indent(),
                strct.name
            ));
            self.indent_level += 1;
            out.push_str(&format!(
                "{}public static var readableContentTypes: [UTType] {{ [.json] }}\n\n",
                self.indent()
            ));
            out.push_str(&format!(
                "{}public init(configuration: ReadConfiguration) throws {{\n",
                self.indent()
            ));
            self.indent_level += 1;
            out.push_str(&format!(
                "{}guard let data = configuration.file.regularFileContents else {{\n",
                self.indent()
            ));
            self.indent_level += 1;
            out.push_str(&format!(
                "{}throw CocoaError(.fileReadCorruptFile)\n",
                self.indent()
            ));
            self.indent_level -= 1;
            out.push_str(&format!("{}}}\n", self.indent()));
            out.push_str(&format!(
                "{}self = try JSONDecoder().decode({}.self, from: data)\n",
                self.indent(),
                strct.name
            ));
            self.indent_level -= 1;
            out.push_str(&format!("{}}}\n\n", self.indent()));
            out.push_str(&format!(
                "{}public func fileWrapper(configuration: WriteConfiguration) throws -> FileWrapper {{\n",
                self.indent()
            ));
            self.indent_level += 1;
            out.push_str(&format!(
                "{}let data = try JSONEncoder().encode(self)\n",
                self.indent()
            ));
            out.push_str(&format!(
                "{}return FileWrapper(regularFileWithContents: data)\n",
                self.indent()
            ));
            self.indent_level -= 1;
            out.push_str(&format!("{}}}\n", self.indent()));
            self.indent_level -= 1;
            out.push_str(&format!("{}}}", self.indent()));
        }

        if strct.is_fskit_volume {
            out.push_str("\n\n");
            out.push_str(&format!(
                "{}public final class {}Volume: FSVolume, FSVolumeOperations {{\n",
                self.indent(),
                strct.name
            ));
            self.indent_level += 1;
            out.push_str(&format!(
                "{}public var state: {}\n\n",
                self.indent(),
                strct.name
            ));
            out.push_str(&format!(
                "{}public init(state: {}) {{\n",
                self.indent(),
                strct.name
            ));
            self.indent_level += 1;
            out.push_str(&format!("{}self.state = state\n", self.indent()));
            self.indent_level -= 1;
            out.push_str(&format!("{}}}\n", self.indent()));
            self.indent_level -= 1;
            out.push_str(&format!("{}}}", self.indent()));
        }

        if strct.is_widget_timeline {
            out.push_str("\n\n");
            out.push_str(&format!(
                "{}extension {}: TimelineProvider {{\n",
                self.indent(),
                strct.name
            ));
            self.indent_level += 1;
            out.push_str(&format!(
                "{}public typealias Entry = SimpleTimelineEntry\n",
                self.indent()
            ));
            self.indent_level -= 1;
            out.push_str(&format!("{}}}", self.indent()));
        }

        out
    }

    pub fn generate_view(&mut self, view: &ViewDef) -> String {
        self.symbols.push(SymbolMapping {
            rust_name: view.name.clone(),
            swift_name: view.name.clone(),
            kind: "view".into(),
            rust_file: view.span.file.clone(),
            rust_line: view.span.start_line,
        });

        let mut out = String::new();
        if self.emit_source_locations && view.span.start_line > 0 && !view.span.file.is_empty() {
            let escaped = view.span.file.replace('\\', "\\\\").replace('"', "\\\"");
            out.push_str(&format!(
                "{}#sourceLocation(file: \"{}\", line: {})\n",
                self.indent(),
                escaped,
                view.span.start_line
            ));
        }

        let vis = if view.is_public { "public " } else { "" };
        out.push_str(&format!("{}struct {}: View {{\n", vis, view.name));
        self.indent_level += 1;

        // Properties
        for prop in &view.properties {
            let p_vis = if prop.is_public { "public " } else { "" };
            let swift_ty = self.map_type(&prop.ty);
            match prop.kind {
                PropertyBindingKind::State => {
                    out.push_str(&format!(
                        "{}@State private var {}: {}\n",
                        self.indent(),
                        prop.name,
                        swift_ty
                    ));
                }
                PropertyBindingKind::Binding => {
                    out.push_str(&format!(
                        "{}@Binding {}var {}: {}\n",
                        self.indent(),
                        p_vis,
                        prop.name,
                        swift_ty
                    ));
                }
                PropertyBindingKind::Observable => {
                    out.push_str(&format!(
                        "{}@Bindable {}var {}: {}\n",
                        self.indent(),
                        p_vis,
                        prop.name,
                        swift_ty
                    ));
                }
                PropertyBindingKind::Plain => {
                    out.push_str(&format!(
                        "{}{}var {}: {}\n",
                        self.indent(),
                        p_vis,
                        prop.name,
                        swift_ty
                    ));
                }
            }
        }

        // Initializer
        if !view.properties.is_empty() {
            out.push('\n');
            let init_params = view
                .properties
                .iter()
                .filter(|p| !matches!(p.kind, PropertyBindingKind::State))
                .map(|p| {
                    if matches!(p.kind, PropertyBindingKind::Binding) {
                        format!("{}: Binding<{}>", p.name, self.map_type(&p.ty))
                    } else {
                        format!("{}: {}", p.name, self.map_type(&p.ty))
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");

            out.push_str(&format!(
                "{}{}init({}) {{\n",
                self.indent(),
                vis,
                init_params
            ));
            self.indent_level += 1;
            for prop in &view.properties {
                match prop.kind {
                    PropertyBindingKind::State => {}
                    PropertyBindingKind::Binding => {
                        out.push_str(&format!(
                            "{}self._{} = {}\n",
                            self.indent(),
                            prop.name,
                            prop.name
                        ));
                    }
                    _ => {
                        out.push_str(&format!(
                            "{}self.{} = {}\n",
                            self.indent(),
                            prop.name,
                            prop.name
                        ));
                    }
                }
            }
            self.indent_level -= 1;
            out.push_str(&format!("{}}}\n", self.indent()));
        }

        // Body
        out.push('\n');
        out.push_str(&format!("{}{}var body: some View {{\n", self.indent(), vis));
        self.indent_level += 1;
        self.emit_view_node(&view.root, &mut out);
        self.indent_level -= 1;
        out.push_str(&format!("{}}}\n", self.indent()));

        self.indent_level -= 1;
        out.push_str(&format!("{}}}", self.indent()));
        out
    }

    fn emit_view_node(&mut self, node: &ViewNode, out: &mut String) {
        match &node.kind {
            ViewKind::VStack { spacing } => {
                if let Some(s) = spacing {
                    out.push_str(&format!("{}VStack(spacing: {}) {{\n", self.indent(), s));
                } else {
                    out.push_str(&format!("{}VStack {{\n", self.indent()));
                }
                self.indent_level += 1;
                for child in &node.children {
                    self.emit_view_node(child, out);
                }
                self.indent_level -= 1;
                out.push_str(&format!("{}}}", self.indent()));
            }
            ViewKind::HStack { spacing } => {
                if let Some(s) = spacing {
                    out.push_str(&format!("{}HStack(spacing: {}) {{\n", self.indent(), s));
                } else {
                    out.push_str(&format!("{}HStack {{\n", self.indent()));
                }
                self.indent_level += 1;
                for child in &node.children {
                    self.emit_view_node(child, out);
                }
                self.indent_level -= 1;
                out.push_str(&format!("{}}}", self.indent()));
            }
            ViewKind::ZStack => {
                out.push_str(&format!("{}ZStack {{\n", self.indent()));
                self.indent_level += 1;
                for child in &node.children {
                    self.emit_view_node(child, out);
                }
                self.indent_level -= 1;
                out.push_str(&format!("{}}}", self.indent()));
            }
            ViewKind::Text(val) => {
                let clean = val.trim_matches('"');
                if clean.starts_with('$') || clean.contains('.') && !clean.contains(' ') {
                    out.push_str(&format!("{}Text({})", self.indent(), clean));
                } else {
                    out.push_str(&format!("{}Text(\"{}\")", self.indent(), clean));
                }
            }
            ViewKind::Button { label, action } => {
                let clean_label = label.trim_matches('"');
                let clean_action = action.trim_matches('"');
                out.push_str(&format!(
                    "{}Button(\"{}\") {{\n",
                    self.indent(),
                    clean_label
                ));
                self.indent_level += 1;
                out.push_str(&format!("{}{}\n", self.indent(), clean_action));
                self.indent_level -= 1;
                out.push_str(&format!("{}}}", self.indent()));
            }
            ViewKind::Toggle { label, binding } => {
                let clean_label = label.trim_matches('"');
                let clean_binding = binding.trim_matches('"');
                let bound = if clean_binding.starts_with('$') {
                    clean_binding.to_string()
                } else {
                    format!("${}", clean_binding)
                };
                out.push_str(&format!(
                    "{}Toggle(\"{}\", isOn: {})",
                    self.indent(),
                    clean_label,
                    bound
                ));
            }
            ViewKind::List => {
                out.push_str(&format!("{}List {{\n", self.indent()));
                self.indent_level += 1;
                for child in &node.children {
                    self.emit_view_node(child, out);
                }
                self.indent_level -= 1;
                out.push_str(&format!("{}}}", self.indent()));
            }
            ViewKind::Spacer => {
                out.push_str(&format!("{}Spacer()", self.indent()));
            }
            ViewKind::Divider => {
                out.push_str(&format!("{}Divider()", self.indent()));
            }
            ViewKind::ForEach { items, id } => {
                let clean_items = items.trim_matches('"');
                if let Some(id_prop) = id {
                    let clean_id = id_prop.trim_matches('"');
                    out.push_str(&format!(
                        "{}ForEach({}, id: \\.{}) {{ item in\n",
                        self.indent(),
                        clean_items,
                        clean_id
                    ));
                } else {
                    out.push_str(&format!(
                        "{}ForEach({}) {{ item in\n",
                        self.indent(),
                        clean_items
                    ));
                }
                self.indent_level += 1;
                for child in &node.children {
                    self.emit_view_node(child, out);
                }
                self.indent_level -= 1;
                out.push_str(&format!("{}}}", self.indent()));
            }
            ViewKind::Custom(name) => {
                let clean_name = name.trim_matches('"');
                out.push_str(&format!("{}{}()", self.indent(), clean_name));
            }
        }

        // Apply modifiers
        for modifier in &node.modifiers {
            let ind = self.indent();
            match modifier {
                ViewModifier::Padding(None) => out.push_str(&format!("\n{}.padding()", ind)),
                ViewModifier::Padding(Some(p)) => {
                    out.push_str(&format!("\n{}.padding({})", ind, p))
                }
                ViewModifier::Font(f) => {
                    let clean = f.trim_matches('"');
                    if clean.starts_with('.') {
                        out.push_str(&format!("\n{}.font({})", ind, clean));
                    } else {
                        out.push_str(&format!("\n{}.font(.{})", ind, clean));
                    }
                }
                ViewModifier::ForegroundColor(c) => {
                    let clean = c.trim_matches('"');
                    if clean.starts_with('.') {
                        out.push_str(&format!("\n{}.foregroundColor({})", ind, clean));
                    } else {
                        out.push_str(&format!("\n{}.foregroundColor(.{})", ind, clean));
                    }
                }
                ViewModifier::Frame { width, height } => {
                    let mut parts = Vec::new();
                    if let Some(w) = width {
                        parts.push(format!("width: {}", w));
                    }
                    if let Some(h) = height {
                        parts.push(format!("height: {}", h));
                    }
                    out.push_str(&format!("\n{}.frame({})", ind, parts.join(", ")));
                }
                ViewModifier::Disabled(cond) => {
                    let clean = cond.trim_matches('"');
                    out.push_str(&format!("\n{}.disabled({})", ind, clean));
                }
                ViewModifier::Background(bg) => {
                    let clean = bg.trim_matches('"');
                    out.push_str(&format!("\n{}.background({})", ind, clean));
                }
            }
        }
        out.push('\n');
    }

    fn generate_impl(&mut self, imp: &ImplBlock) -> String {
        let mut out = String::new();
        if self.emit_source_locations && imp.span.start_line > 0 && !imp.span.file.is_empty() {
            let escaped = imp.span.file.replace('\\', "\\\\").replace('"', "\\\"");
            out.push_str(&format!(
                "{}#sourceLocation(file: \"{}\", line: {})\n",
                self.indent(),
                escaped,
                imp.span.start_line
            ));
        }
        out.push_str(&format!(
            "{}extension {} {{\n",
            self.indent(),
            imp.target_type
        ));
        self.indent_level += 1;
        let mut has_emitted = false;
        for constant in &imp.constants {
            if has_emitted {
                out.push_str("\n\n");
            }
            has_emitted = true;
            self.emit_doc_comments(&mut out, &constant.doc_comments);
            let vis = if constant.is_public { "public " } else { "" };
            let val_str = self.generate_expr(&constant.value);
            out.push_str(&format!(
                "{}{}static let {}: {} = {}",
                self.indent(),
                vis,
                constant.name,
                self.map_type(&constant.ty),
                val_str
            ));
        }
        for method in &imp.methods {
            if has_emitted {
                out.push_str("\n\n");
            }
            has_emitted = true;
            out.push_str(&self.generate_method(&imp.target_type, method, false));
        }
        out.push('\n');
        self.indent_level -= 1;
        out.push_str(&format!("{}}}", self.indent()));
        out
    }

    fn generate_trait(&mut self, trt: &TraitDef) -> String {
        self.symbols.push(SymbolMapping {
            rust_name: trt.name.clone(),
            swift_name: trt.name.clone(),
            kind: "trait".into(),
            rust_file: trt.span.file.clone(),
            rust_line: trt.span.start_line,
        });

        let mut out = String::new();
        if self.emit_source_locations && trt.span.start_line > 0 && !trt.span.file.is_empty() {
            let escaped = trt.span.file.replace('\\', "\\\\").replace('"', "\\\"");
            out.push_str(&format!(
                "{}#sourceLocation(file: \"{}\", line: {})\n",
                self.indent(),
                escaped,
                trt.span.start_line
            ));
        }
        self.emit_doc_comments(&mut out, &trt.doc_comments);
        let vis = if trt.is_public { "public " } else { "" };
        out.push_str(&format!(
            "{}{}protocol {} {{\n",
            self.indent(),
            vis,
            trt.name
        ));
        self.indent_level += 1;
        for assoc in &trt.associated_types {
            if assoc.bounds.is_empty() {
                out.push_str(&format!("{}associatedtype {}\n", self.indent(), assoc.name));
            } else {
                out.push_str(&format!(
                    "{}associatedtype {}: {}\n",
                    self.indent(),
                    assoc.name,
                    assoc.bounds.join(", ")
                ));
            }
        }
        if !trt.associated_types.is_empty() && !trt.methods.is_empty() {
            out.push('\n');
        }
        for method in &trt.methods {
            self.emit_doc_comments(&mut out, &method.doc_comments);
            let mutating_kw = if matches!(method.receiver, Some(ReceiverKind::MutRef)) {
                "mutating "
            } else {
                ""
            };
            let params_str = method
                .params
                .iter()
                .map(|p| {
                    if p.is_exclusive_ref() {
                        let inner_ty = match &p.ty {
                            Type::Ref { inner, .. } => self.map_type(inner),
                            other => self.map_type(other),
                        };
                        format!("_ {}: inout {}", p.name, inner_ty)
                    } else {
                        format!("_ {}: {}", p.name, self.map_type(&p.ty))
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");

            let ret_str = if method.return_type == Type::Unit {
                String::new()
            } else {
                format!(" -> {}", self.map_type(&method.return_type))
            };

            let async_kw = if method.is_async { " async" } else { "" };
            out.push_str(&format!(
                "{}{}func {}({}){}{}\n",
                self.indent(),
                mutating_kw,
                method.name,
                params_str,
                async_kw,
                ret_str
            ));
        }
        self.indent_level -= 1;
        out.push_str(&format!("{}}}", self.indent()));

        // Default protocol implementations in extension
        let default_methods: Vec<&TraitMethod> = trt
            .methods
            .iter()
            .filter(|m| m.default_body.is_some())
            .collect();

        if !default_methods.is_empty() {
            out.push_str(&format!(
                "\n\n{}{}extension {} {{\n",
                self.indent(),
                vis,
                trt.name
            ));
            self.indent_level += 1;
            for (i, method) in default_methods.iter().enumerate() {
                if i > 0 {
                    out.push_str("\n\n");
                }
                let body = method.default_body.as_ref().unwrap();
                let mutating_kw = if matches!(method.receiver, Some(ReceiverKind::MutRef)) {
                    "mutating "
                } else {
                    ""
                };
                let params_str = method
                    .params
                    .iter()
                    .map(|p| {
                        if p.is_exclusive_ref() {
                            let inner_ty = match &p.ty {
                                Type::Ref { inner, .. } => self.map_type(inner),
                                other => self.map_type(other),
                            };
                            format!("_ {}: inout {}", p.name, inner_ty)
                        } else {
                            format!("_ {}: {}", p.name, self.map_type(&p.ty))
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", ");

                let ret_str = if method.return_type == Type::Unit {
                    String::new()
                } else {
                    format!(" -> {}", self.map_type(&method.return_type))
                };

                let async_kw = if method.is_async { " async" } else { "" };
                out.push_str(&format!(
                    "{}{}func {}({}){}{} {{\n",
                    self.indent(),
                    mutating_kw,
                    method.name,
                    params_str,
                    async_kw,
                    ret_str
                ));
                self.indent_level += 1;
                for p in &method.params {
                    if p.is_mutable_binding() {
                        out.push_str(&format!("{}var {} = {}\n", self.indent(), p.name, p.name));
                    }
                }

                let body_str = self.generate_expr(body);
                if method.return_type != Type::Unit {
                    if body_str.starts_with("if ") || body_str.starts_with("switch ") {
                        out.push_str(&format!("{}{}\n", self.indent(), body_str));
                    } else {
                        out.push_str(&format!("{}return {}\n", self.indent(), body_str));
                    }
                } else {
                    out.push_str(&format!("{}{}\n", self.indent(), body_str));
                }
                self.indent_level -= 1;
                out.push_str(&format!("{}}}", self.indent()));
            }
            self.indent_level -= 1;
            out.push_str(&format!("\n{}}}", self.indent()));
        }

        out
    }

    fn generate_trait_impl(&mut self, timpl: &TraitImpl) -> String {
        let mut out = String::new();
        if self.emit_source_locations && timpl.span.start_line > 0 && !timpl.span.file.is_empty() {
            let escaped = timpl.span.file.replace('\\', "\\\\").replace('"', "\\\"");
            out.push_str(&format!(
                "{}#sourceLocation(file: \"{}\", line: {})\n",
                self.indent(),
                escaped,
                timpl.span.start_line
            ));
        }
        let where_suffix = match &timpl.where_clause {
            Some(w) => format!(" where {}", w),
            None => String::new(),
        };
        out.push_str(&format!(
            "{}extension {}: {}{} {{\n",
            self.indent(),
            timpl.target_type,
            timpl.trait_name,
            where_suffix
        ));
        self.indent_level += 1;
        for alias in &timpl.type_aliases {
            self.emit_doc_comments(&mut out, &alias.doc_comments);
            out.push_str(&format!(
                "{}public typealias {} = {}\n",
                self.indent(),
                alias.name,
                self.map_type(&alias.target)
            ));
        }
        if !timpl.type_aliases.is_empty() && !timpl.methods.is_empty() {
            out.push('\n');
        }
        for (i, method) in timpl.methods.iter().enumerate() {
            if i > 0 {
                out.push_str("\n\n");
            }
            out.push_str(&self.generate_method(&timpl.target_type, method, true));
        }
        out.push('\n');
        self.indent_level -= 1;
        out.push_str(&format!("{}}}", self.indent()));
        out
    }

    fn generate_method(
        &mut self,
        target_type: &str,
        method: &Method,
        is_protocol_conformance: bool,
    ) -> String {
        self.symbols.push(SymbolMapping {
            rust_name: format!("{}::{}", target_type, method.name),
            swift_name: format!("{}.{}", target_type, method.name),
            kind: "method".into(),
            rust_file: method.span.file.clone(),
            rust_line: method.span.start_line,
        });

        let mut out = String::new();
        if self.emit_source_locations && method.span.start_line > 0 && !method.span.file.is_empty()
        {
            let escaped = method.span.file.replace('\\', "\\\\").replace('"', "\\\"");
            out.push_str(&format!(
                "{}#sourceLocation(file: \"{}\", line: {})\n",
                self.indent(),
                escaped,
                method.span.start_line
            ));
        }
        self.emit_doc_comments(&mut out, &method.doc_comments);
        let vis = if is_protocol_conformance || method.is_public {
            "public "
        } else {
            ""
        };
        let is_class_method = self.observable_types.contains(target_type);
        let modifier = match method.receiver {
            None => "static ",
            Some(ReceiverKind::MutRef) => {
                if is_class_method {
                    ""
                } else {
                    "mutating "
                }
            }
            Some(ReceiverKind::Ref) | Some(ReceiverKind::Value) => "",
        };

        let params_str = method
            .params
            .iter()
            .map(|p| {
                if p.is_exclusive_ref() {
                    let inner_ty = match &p.ty {
                        Type::Ref { inner, .. } => self.map_type(inner),
                        other => self.map_type(other),
                    };
                    format!("_ {}: inout {}", p.name, inner_ty)
                } else {
                    format!("_ {}: {}", p.name, self.map_type(&p.ty))
                }
            })
            .collect::<Vec<_>>()
            .join(", ");

        let ret_str = if method.return_type == Type::Unit {
            "".into()
        } else {
            format!(" -> {}", self.map_type(&method.return_type))
        };

        let async_kw = if method.is_async { " async" } else { "" };
        out.push_str(&format!(
            "{}{}{}func {}({}){}{} {{\n",
            self.indent(),
            vis,
            modifier,
            method.name,
            params_str,
            async_kw,
            ret_str
        ));

        self.indent_level += 1;
        for p in &method.params {
            if p.is_mutable_binding() {
                out.push_str(&format!("{}var {} = {}\n", self.indent(), p.name, p.name));
            }
        }
        out.push_str(&self.generate_function_body(&method.body, method.return_type != Type::Unit));
        self.indent_level -= 1;

        out.push_str(&format!("{}}}", self.indent()));
        out
    }

    fn generate_function(&mut self, func: &Function) -> String {
        self.symbols.push(SymbolMapping {
            rust_name: func.name.clone(),
            swift_name: func.name.clone(),
            kind: "function".into(),
            rust_file: func.span.file.clone(),
            rust_line: func.span.start_line,
        });

        let mut out = String::new();
        if self.emit_source_locations && func.span.start_line > 0 && !func.span.file.is_empty() {
            let escaped = func.span.file.replace('\\', "\\\\").replace('"', "\\\"");
            out.push_str(&format!(
                "{}#sourceLocation(file: \"{}\", line: {})\n",
                self.indent(),
                escaped,
                func.span.start_line
            ));
        }
        self.emit_doc_comments(&mut out, &func.doc_comments);
        let vis = if func.is_public { "public " } else { "" };
        let params_str = func
            .params
            .iter()
            .map(|p| {
                if p.is_exclusive_ref() {
                    let inner_ty = match &p.ty {
                        Type::Ref { inner, .. } => self.map_type(inner),
                        other => self.map_type(other),
                    };
                    format!("_ {}: inout {}", p.name, inner_ty)
                } else {
                    format!("_ {}: {}", p.name, self.map_type(&p.ty))
                }
            })
            .collect::<Vec<_>>()
            .join(", ");

        let ret_str = if func.return_type == Type::Unit {
            "".into()
        } else {
            format!(" -> {}", self.map_type(&func.return_type))
        };

        let async_kw = if func.is_async { " async" } else { "" };
        out.push_str(&format!(
            "{}{}func {}({}){}{} {{\n",
            self.indent(),
            vis,
            func.name,
            params_str,
            async_kw,
            ret_str
        ));

        self.indent_level += 1;
        for p in &func.params {
            if p.is_mutable_binding() {
                out.push_str(&format!("{}var {} = {}\n", self.indent(), p.name, p.name));
            }
        }
        out.push_str(&self.generate_function_body(&func.body, func.return_type != Type::Unit));
        self.indent_level -= 1;

        out.push_str(&format!("{}}}", self.indent()));
        out
    }

    fn generate_constant(&mut self, c: &ConstantDef) -> String {
        self.symbols.push(SymbolMapping {
            rust_name: c.name.clone(),
            swift_name: c.name.clone(),
            kind: "constant".into(),
            rust_file: c.span.file.clone(),
            rust_line: c.span.start_line,
        });

        let mut out = String::new();
        if self.emit_source_locations && c.span.start_line > 0 && !c.span.file.is_empty() {
            let escaped = c.span.file.replace('\\', "\\\\").replace('"', "\\\"");
            out.push_str(&format!(
                "{}#sourceLocation(file: \"{}\", line: {})\n",
                self.indent(),
                escaped,
                c.span.start_line
            ));
        }
        self.emit_doc_comments(&mut out, &c.doc_comments);
        let vis = if c.is_public { "public " } else { "" };
        let ty_str = self.map_type(&c.ty);
        let val_str = self.generate_expr(&c.value);
        out.push_str(&format!(
            "{}{}let {}: {} = {}",
            self.indent(),
            vis,
            c.name,
            ty_str,
            val_str
        ));
        out
    }

    fn generate_type_alias(&mut self, ta: &TypeAlias) -> String {
        self.symbols.push(SymbolMapping {
            rust_name: ta.name.clone(),
            swift_name: ta.name.clone(),
            kind: "typealias".into(),
            rust_file: ta.span.file.clone(),
            rust_line: ta.span.start_line,
        });

        let mut out = String::new();
        if self.emit_source_locations && ta.span.start_line > 0 && !ta.span.file.is_empty() {
            let escaped = ta.span.file.replace('\\', "\\\\").replace('"', "\\\"");
            out.push_str(&format!(
                "{}#sourceLocation(file: \"{}\", line: {})\n",
                self.indent(),
                escaped,
                ta.span.start_line
            ));
        }
        self.emit_doc_comments(&mut out, &ta.doc_comments);
        let vis = if ta.is_public { "public " } else { "" };
        let target_str = self.map_type(&ta.target);
        out.push_str(&format!(
            "{}{}typealias {} = {}",
            self.indent(),
            vis,
            ta.name,
            target_str
        ));
        out
    }

    fn generate_function_body(&mut self, expr: &Expr, returns_value: bool) -> String {
        match &expr.kind {
            ExprKind::Block {
                stmts,
                expr: final_expr,
            } => {
                let mut out = String::new();
                for stmt in stmts {
                    out.push_str(&self.generate_stmt(stmt));
                }
                if let Some(final_e) = final_expr {
                    if returns_value {
                        out.push_str(&self.generate_return_expr(final_e));
                    } else {
                        out.push_str(&self.generate_stmt(&Stmt::Expr(*final_e.clone())));
                    }
                }
                out
            }
            ExprKind::Match { .. } if returns_value => self.generate_return_expr(expr),
            _ => {
                if returns_value {
                    self.generate_return_expr(expr)
                } else {
                    format!("{}{}\n", self.indent(), self.generate_expr(expr))
                }
            }
        }
    }

    fn generate_stmt(&mut self, stmt: &Stmt) -> String {
        match stmt {
            Stmt::Let {
                name,
                mutable,
                ty,
                init,
            } => {
                let keyword = if *mutable { "var" } else { "let" };
                let ty_ann = ty
                    .as_ref()
                    .map(|t| format!(": {}", self.map_type(t)))
                    .unwrap_or_default();
                let init_str = init
                    .as_ref()
                    .map(|i| format!(" = {}", self.generate_expr(i)))
                    .unwrap_or_default();
                format!(
                    "{}{} {}{}{}\n",
                    self.indent(),
                    keyword,
                    name,
                    ty_ann,
                    init_str
                )
            }
            Stmt::Assign { target, value } => {
                format!(
                    "{}{} = {}\n",
                    self.indent(),
                    self.generate_expr(target),
                    self.generate_expr(value)
                )
            }
            Stmt::Expr(Expr {
                kind:
                    ExprKind::If {
                        cond,
                        then_branch,
                        else_branch,
                    },
                ..
            }) => {
                let mut out = String::new();
                out.push_str(&format!(
                    "{}if {} {{\n",
                    self.indent(),
                    self.generate_expr(cond)
                ));
                self.indent_level += 1;
                out.push_str(&self.generate_function_body(then_branch, false));
                self.indent_level -= 1;
                if let Some(eb) = else_branch {
                    out.push_str(&format!("{}}} else {{\n", self.indent()));
                    self.indent_level += 1;
                    out.push_str(&self.generate_function_body(eb, false));
                    self.indent_level -= 1;
                }
                out.push_str(&format!("{}}}\n", self.indent()));
                out
            }
            Stmt::Expr(Expr {
                kind: ExprKind::Panic(msg),
                ..
            }) => {
                if let Some(msg) = msg {
                    format!("{}fatalError(\"{}\")\n", self.indent(), msg)
                } else {
                    format!("{}fatalError(\"panic\")\n", self.indent())
                }
            }
            Stmt::Expr(Expr {
                kind:
                    ExprKind::For {
                        pattern,
                        iter,
                        body,
                    },
                ..
            }) => {
                let pat_str = self.generate_for_pattern(pattern);
                let iter_str = self.generate_expr(iter);
                let mut out = format!("{}for {} in {} {{\n", self.indent(), pat_str, iter_str);
                self.indent_level += 1;
                out.push_str(&self.generate_function_body(body, false));
                self.indent_level -= 1;
                out.push_str(&format!("{}}}\n", self.indent()));
                out
            }
            Stmt::Expr(Expr {
                kind: ExprKind::While { cond, body },
                ..
            }) => {
                let cond_str = self.generate_expr(cond);
                let mut out = format!("{}while {} {{\n", self.indent(), cond_str);
                self.indent_level += 1;
                out.push_str(&self.generate_function_body(body, false));
                self.indent_level -= 1;
                out.push_str(&format!("{}}}\n", self.indent()));
                out
            }
            Stmt::Expr(Expr {
                kind: ExprKind::Loop { body },
                ..
            }) => {
                let mut out = format!("{}while true {{\n", self.indent());
                self.indent_level += 1;
                out.push_str(&self.generate_function_body(body, false));
                self.indent_level -= 1;
                out.push_str(&format!("{}}}\n", self.indent()));
                out
            }
            Stmt::Expr(expr) => {
                format!("{}{}\n", self.indent(), self.generate_expr(expr))
            }
            Stmt::Assert { condition, message } => {
                let cond_str = self.generate_expr(condition);
                if let Some(msg) = message {
                    format!("{}precondition({}, \"{}\")\n", self.indent(), cond_str, msg)
                } else {
                    format!("{}precondition({})\n", self.indent(), cond_str)
                }
            }
            Stmt::DebugAssert { condition, message } => {
                let cond_str = self.generate_expr(condition);
                if let Some(msg) = message {
                    format!("{}assert({}, \"{}\")\n", self.indent(), cond_str, msg)
                } else {
                    format!("{}assert({})\n", self.indent(), cond_str)
                }
            }
            Stmt::AssertEq {
                left,
                right,
                message,
            } => {
                let l_str = self.generate_expr(left);
                let r_str = self.generate_expr(right);
                if let Some(msg) = message {
                    format!(
                        "{}precondition({} == {}, \"{}\")\n",
                        self.indent(),
                        l_str,
                        r_str,
                        msg
                    )
                } else {
                    format!("{}precondition({} == {})\n", self.indent(), l_str, r_str)
                }
            }
            Stmt::Panic { message } => {
                if let Some(msg) = message {
                    format!("{}fatalError(\"{}\")\n", self.indent(), msg)
                } else {
                    format!("{}fatalError(\"panic\")\n", self.indent())
                }
            }
        }
    }

    fn generate_return_expr(&mut self, expr: &Expr) -> String {
        match &expr.kind {
            ExprKind::Match { expr: target, arms } => {
                let mut out = String::new();
                out.push_str(&format!(
                    "{}switch {} {{\n",
                    self.indent(),
                    self.generate_expr(target)
                ));
                self.indent_level += 1;
                for arm in arms {
                    out.push_str(&self.generate_match_arm(arm, true));
                }
                self.indent_level -= 1;
                out.push_str(&format!("{}}}\n", self.indent()));
                out
            }
            ExprKind::Block { .. } => self.generate_function_body(expr, true),
            ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                let mut out = String::new();
                out.push_str(&format!(
                    "{}if {} {{\n",
                    self.indent(),
                    self.generate_expr(cond)
                ));
                self.indent_level += 1;
                out.push_str(&self.generate_function_body(then_branch, true));
                self.indent_level -= 1;
                if let Some(eb) = else_branch {
                    out.push_str(&format!("{}}} else {{\n", self.indent()));
                    self.indent_level += 1;
                    out.push_str(&self.generate_function_body(eb, true));
                    self.indent_level -= 1;
                }
                out.push_str(&format!("{}}}\n", self.indent()));
                out
            }
            ExprKind::Panic(message) => {
                if let Some(msg) = message {
                    format!("{}fatalError(\"{}\")\n", self.indent(), msg)
                } else {
                    format!("{}fatalError(\"panic\")\n", self.indent())
                }
            }
            ExprKind::Return(Some(inner)) => self.generate_return_expr(inner),
            _ => {
                format!("{}return {}\n", self.indent(), self.generate_expr(expr))
            }
        }
    }

    fn generate_match_arm(&mut self, arm: &MatchArm, is_return: bool) -> String {
        let mut out = String::new();
        let pat_str = self.generate_pattern(&arm.pattern);
        let case_label = if pat_str == "_" {
            "default".into()
        } else {
            format!("case {}", pat_str)
        };

        if let Some(guard) = &arm.guard {
            out.push_str(&format!(
                "{}{} where {}:\n",
                self.indent(),
                case_label,
                self.generate_expr(guard)
            ));
        } else {
            out.push_str(&format!("{}{}:\n", self.indent(), case_label));
        }

        self.indent_level += 1;
        if is_return {
            out.push_str(&self.generate_return_expr(&arm.body));
        } else {
            out.push_str(&format!(
                "{}{}\n",
                self.indent(),
                self.generate_expr(&arm.body)
            ));
        }
        self.indent_level -= 1;

        out
    }

    fn generate_for_pattern(&self, pat: &Pattern) -> String {
        match pat {
            Pattern::Wildcard => "_".into(),
            Pattern::Variable(v) => v.clone(),
            Pattern::Tuple(elems) => {
                let inner = elems
                    .iter()
                    .map(|p| self.generate_for_pattern(p))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("({})", inner)
            }
            _ => self.generate_pattern(pat),
        }
    }

    fn generate_pattern(&self, pat: &Pattern) -> String {
        match pat {
            Pattern::Wildcard => "_".into(),
            Pattern::Literal(lit) => self.generate_literal(lit),
            Pattern::Variable(v) => {
                if v == "None" {
                    ".none".into()
                } else {
                    format!("let {}", v)
                }
            }
            Pattern::EnumVariant {
                enum_name: _,
                variant,
            } => {
                let v = match variant.as_str() {
                    "None" => "none",
                    other => other,
                };
                format!(".{}", v)
            }
            Pattern::EnumVariantPayload {
                enum_name: _,
                variant,
                patterns,
            } => {
                let v = match variant.as_str() {
                    "Some" => "some",
                    "Ok" => "success",
                    "Err" => "failure",
                    other => other,
                };
                let inner = patterns
                    .iter()
                    .map(|p| self.generate_pattern(p))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(".{}({})", v, inner)
            }
            Pattern::Tuple(elems) => {
                let inner = elems
                    .iter()
                    .map(|p| self.generate_pattern(p))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("({})", inner)
            }
            Pattern::Struct { name, fields } => {
                let variant = name.split('.').next_back().unwrap_or(name);
                let inner = fields
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k, self.generate_pattern(v)))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(".{}({})", variant, inner)
            }
        }
    }

    fn generate_expr(&self, expr: &Expr) -> String {
        match &expr.kind {
            ExprKind::Literal(lit) => self.generate_literal(lit),
            ExprKind::Variable(var) => {
                if var == "None" {
                    "nil".into()
                } else if var.contains('.') {
                    let parts: Vec<&str> = var.split('.').collect();
                    if parts.len() == 2 {
                        format!("{}.{}", parts[0], parts[1])
                    } else {
                        var.clone()
                    }
                } else {
                    var.clone()
                }
            }
            ExprKind::Binary { op, left, right } => {
                let l_str = self.generate_expr(left);
                let r_str = self.generate_expr(right);
                match op {
                    BinaryOp::Arithmetic { op: aop, mode } => {
                        let op_sym = match (aop, mode) {
                            (IntegerOp::Add, IntegerMode::Wrapping) => "&+",
                            (IntegerOp::Sub, IntegerMode::Wrapping) => "&-",
                            (IntegerOp::Mul, IntegerMode::Wrapping) => "&*",
                            (IntegerOp::Add, _) => "+",
                            (IntegerOp::Sub, _) => "-",
                            (IntegerOp::Mul, _) => "*",
                            (IntegerOp::Div, _) => "/",
                            (IntegerOp::Rem, _) => "%",
                            (IntegerOp::BitAnd, _) => "&",
                            (IntegerOp::BitOr, _) => "|",
                            (IntegerOp::BitXor, _) => "^",
                            (IntegerOp::Shl, _) => "<<",
                            (IntegerOp::Shr, _) => ">>",
                        };
                        format!("({} {} {})", l_str, op_sym, r_str)
                    }
                    BinaryOp::Comparison(cmp) => {
                        let sym = match cmp {
                            CmpOp::Eq => "==",
                            CmpOp::Ne => "!=",
                            CmpOp::Lt => "<",
                            CmpOp::Le => "<=",
                            CmpOp::Gt => ">",
                            CmpOp::Ge => ">=",
                        };
                        format!("({} {} {})", l_str, sym, r_str)
                    }
                    BinaryOp::Logical(log) => {
                        let sym = match log {
                            LogicalOp::And => "&&",
                            LogicalOp::Or => "||",
                        };
                        format!("({} {} {})", l_str, sym, r_str)
                    }
                }
            }
            ExprKind::Unary { op, expr } => {
                let e_str = self.generate_expr(expr);
                match op {
                    UnaryOp::Not => format!("!({})", e_str),
                    UnaryOp::Neg => format!("-({})", e_str),
                    UnaryOp::Deref => e_str,
                }
            }
            ExprKind::Call { func, args } => {
                if func == "Vec.new" && args.is_empty() {
                    return "[]".into();
                }
                let swift_func = match func.as_str() {
                    "Some" => ".some",
                    "Ok" => ".success",
                    "Err" => ".failure",
                    other => other,
                };
                let param_inouts = self.fn_inouts.get(func).cloned();
                let args_str = args
                    .iter()
                    .enumerate()
                    .map(|(i, a)| {
                        let mut gen = self.generate_expr(a);
                        let expects_inout = param_inouts
                            .as_ref()
                            .and_then(|p| p.get(i))
                            .copied()
                            .unwrap_or(false);
                        if expects_inout && !gen.starts_with('&') {
                            gen = format!("&{}", gen);
                        }
                        gen
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{}({})", swift_func, args_str)
            }
            ExprKind::MethodCall {
                receiver,
                method,
                args,
            } => {
                let r_str = self.generate_expr(receiver);
                if method == "clone" && args.is_empty() {
                    return r_str;
                }
                let swift_method = match method.as_str() {
                    "push" => "append",
                    "pop" => "popLast",
                    "clear" => "removeAll",
                    "len" => "len",
                    "is_empty" => "is_empty",
                    "is_some" => "is_some",
                    "is_none" => "is_none",
                    "is_ok" => "is_ok",
                    "is_err" => "is_err",
                    "unwrap" => "unwrap",
                    other => other,
                };
                let args_str = args
                    .iter()
                    .map(|a| self.generate_expr(a))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{}.{}({})", r_str, swift_method, args_str)
            }
            ExprKind::FieldAccess { receiver, field } => {
                let f = if field == "raw_value" {
                    "rawValue"
                } else {
                    field.as_str()
                };
                format!("{}.{}", self.generate_expr(receiver), f)
            }
            ExprKind::StructInit { name, fields } => {
                let fields_str = fields
                    .iter()
                    .map(|(k, v)| {
                        let k_str = if k == "raw_value" {
                            "rawValue"
                        } else {
                            k.as_str()
                        };
                        format!("{}: {}", k_str, self.generate_expr(v))
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("{}({})", name, fields_str)
            }
            ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                if let Some(eb) = else_branch {
                    format!(
                        "({} ? {} : {})",
                        self.generate_expr(cond),
                        self.generate_expr(then_branch),
                        self.generate_expr(eb)
                    )
                } else {
                    format!("if {} {{ ... }}", self.generate_expr(cond))
                }
            }
            ExprKind::Return(Some(inner)) => {
                format!("return {}", self.generate_expr(inner))
            }
            ExprKind::Return(None) => "return".into(),
            ExprKind::Break(_) => "break".into(),
            ExprKind::Continue => "continue".into(),
            ExprKind::Tuple(elems) => {
                let inner = elems
                    .iter()
                    .map(|e| self.generate_expr(e))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("({})", inner)
            }
            ExprKind::Array(elems) => {
                let inner = elems
                    .iter()
                    .map(|e| self.generate_expr(e))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("[{}]", inner)
            }
            ExprKind::Index { expr, index } => {
                let e_str = self.generate_expr(expr);
                let idx_str = self.generate_expr(index);
                format!("{}[Int({})]", e_str, idx_str)
            }
            ExprKind::Slice { expr, start, end } => {
                let e_str = self.generate_expr(expr);
                let start_str = start
                    .as_ref()
                    .map(|s| format!("Int({})", self.generate_expr(s)))
                    .unwrap_or_else(|| "0".into());
                let end_str = end
                    .as_ref()
                    .map(|e| format!("Int({})", self.generate_expr(e)))
                    .unwrap_or_else(|| format!("{}.utf8.count", e_str));
                format!("{}.byteSlice({}, {})", e_str, start_str, end_str)
            }
            ExprKind::Borrow { expr, mutable } => {
                let e_str = self.generate_expr(expr);
                if *mutable {
                    format!("&{}", e_str)
                } else {
                    e_str
                }
            }
            ExprKind::Block {
                stmts: _,
                expr: Some(final_e),
            } => self.generate_expr(final_e),
            ExprKind::Panic(message) => {
                if let Some(msg) = message {
                    format!("fatalError(\"{}\")", msg)
                } else {
                    "fatalError(\"panic\")".into()
                }
            }
            ExprKind::StringFormat { template, args } => {
                self.generate_string_format(template, args)
            }
            ExprKind::Await(inner) => {
                format!("await {}", self.generate_expr(inner))
            }
            _ => "/* unsupported inline expression */".into(),
        }
    }

    fn generate_string_format(&self, template: &str, args: &[Expr]) -> String {
        let mut result = String::from("\"");
        let mut chars = template.chars().peekable();
        let mut arg_idx = 0;

        while let Some(ch) = chars.next() {
            if ch == '{' {
                if chars.peek() == Some(&'{') {
                    chars.next();
                    result.push('{');
                } else {
                    let mut spec = String::new();
                    while let Some(&c) = chars.peek() {
                        if c == '}' {
                            chars.next();
                            break;
                        }
                        spec.push(chars.next().unwrap());
                    }
                    if spec.is_empty() || spec == ":?" || spec == "?" {
                        if arg_idx < args.len() {
                            let arg_str = self.generate_expr(&args[arg_idx]);
                            result.push_str(&format!("\\({})", arg_str));
                            arg_idx += 1;
                        } else {
                            result.push_str("{}");
                        }
                    } else if !args.is_empty() && arg_idx < args.len() {
                        let arg_str = self.generate_expr(&args[arg_idx]);
                        result.push_str(&format!("\\({})", arg_str));
                        arg_idx += 1;
                    } else {
                        let var_name = spec.split(':').next().unwrap_or(&spec).trim();
                        result.push_str(&format!("\\({})", var_name));
                    }
                }
            } else if ch == '}' {
                if chars.peek() == Some(&'}') {
                    chars.next();
                    result.push('}');
                } else {
                    result.push('}');
                }
            } else if ch == '"' {
                result.push_str("\\\"");
            } else if ch == '\\' {
                result.push_str("\\\\");
            } else {
                result.push(ch);
            }
        }
        result.push('"');
        result
    }

    fn generate_literal(&self, lit: &Literal) -> String {
        match lit {
            Literal::Int { value, ty: _ } => value.to_string(),
            Literal::UInt { value, ty: _ } => value.to_string(),
            Literal::Bool(b) => {
                if *b {
                    "true".into()
                } else {
                    "false".into()
                }
            }
            Literal::String(s) => {
                let mut escaped = String::with_capacity(s.len() + 2);
                escaped.push('"');
                for c in s.chars() {
                    match c {
                        '\\' => escaped.push_str("\\\\"),
                        '"' => escaped.push_str("\\\""),
                        '\n' => escaped.push_str("\\n"),
                        '\r' => escaped.push_str("\\r"),
                        '\t' => escaped.push_str("\\t"),
                        '\0' => escaped.push_str("\\0"),
                        other => escaped.push(other),
                    }
                }
                escaped.push('"');
                escaped
            }
            Literal::Unit => "()".into(),
        }
    }

    /// Emits a complete, buildable Swift package layout on disk.
    pub fn emit_package(
        &mut self,
        module: &Module,
        output_dir: &Path,
        package_name: &str,
    ) -> std::io::Result<PackageManifest> {
        let sources_dir = output_dir.join("Sources").join(package_name);
        fs::create_dir_all(&sources_dir)?;

        let (swift_code, symbols) = self.generate_module(module);
        let swift_file_path = sources_dir.join(format!("{}.swift", module.name));
        fs::write(&swift_file_path, swift_code)?;

        // Package.swift
        let package_swift = format!(
            r#"// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "{pkg}",
    platforms: [
        .macOS(.v14),
        .iOS(.v17),
        .tvOS(.v17),
        .watchOS(.v10),
        .visionOS(.v1),
    ],
    products: [
        .library(
            name: "{pkg}",
            targets: ["{pkg}"]
        ),
    ],
    targets: [
        .target(
            name: "{pkg}",
            path: "Sources/{pkg}"
        ),
        .testTarget(
            name: "{pkg}Tests",
            dependencies: ["{pkg}"],
            path: "Tests/{pkg}Tests"
        ),
    ]
)
"#,
            pkg = package_name
        );
        fs::write(output_dir.join("Package.swift"), package_swift)?;

        // Runtime shims if needed
        let runtime_shims = r#"// Minimal Swift compatibility shims for rust-swift compiler
import Foundation

public struct RustCompat {
    public static func saturatingAdd<T: FixedWidthInteger>(_ a: T, _ b: T) -> T {
        let (res, overflow) = a.addingReportingOverflow(b)
        if overflow {
            return b >= 0 ? T.max : T.min
        }
        return res
    }

    public static func saturatingSub<T: FixedWidthInteger>(_ a: T, _ b: T) -> T {
        let (res, overflow) = a.subtractingReportingOverflow(b)
        if overflow {
            return b >= 0 ? T.min : T.max
        }
        return res
    }
}

extension String {
    public static func from(_ s: String) -> String {
        return s
    }

    public static func from(_ s: Substring) -> String {
        return String(s)
    }

    public func byteSlice(_ start: Int, _ end: Int) -> String {
        precondition(start >= 0 && end >= start && end <= self.utf8.count, "String slice index out of bounds")
        let startIdx = self.utf8.index(self.utf8.startIndex, offsetBy: start)
        let endIdx = self.utf8.index(self.utf8.startIndex, offsetBy: end)
        guard let sub = String(self.utf8[startIdx..<endIdx]) else {
            fatalError("String slice boundary is not a valid UTF-8 code point")
        }
        return sub
    }

    public func len() -> Int {
        return self.utf8.count
    }

    public func is_empty() -> Bool {
        return self.isEmpty
    }

    public func to_string() -> String {
        return self
    }
}

extension Array {
    public mutating func push(_ element: Element) {
        self.append(element)
    }

    public mutating func pop() -> Element? {
        return self.popLast()
    }

    public mutating func clear() {
        self.removeAll()
    }

    public func len() -> Int {
        return self.count
    }

    public func is_empty() -> Bool {
        return self.isEmpty
    }
}

extension Optional {
    public func is_some() -> Bool {
        return self != nil
    }

    public func is_none() -> Bool {
        return self == nil
    }

    public func unwrap() -> Wrapped {
        guard let value = self else {
            fatalError("Called unwrap() on a None value")
        }
        return value
    }
}

extension Result {
    public var isOk: Bool {
        switch self {
        case .success: return true
        case .failure: return false
        }
    }

    public var isErr: Bool {
        switch self {
        case .failure: return true
        case .success: return false
        }
    }

    public func is_ok() -> Bool {
        return self.isOk
    }

    public func is_err() -> Bool {
        return self.isErr
    }

    public func unwrap() -> Success {
        switch self {
        case .success(let val): return val
        case .failure(let err): fatalError("Called unwrap() on an Err value: \(err)")
        }
    }
}

public protocol Clone {}
extension Int: Clone {}
extension Int8: Clone {}
extension Int16: Clone {}
extension Int32: Clone {}
extension Int64: Clone {}
extension UInt: Clone {}
extension UInt8: Clone {}
extension UInt16: Clone {}
extension UInt32: Clone {}
extension UInt64: Clone {}
extension String: Clone {}
extension Bool: Clone {}
extension Float: Clone {}
extension Double: Clone {}
extension Array: Clone {}
extension Dictionary: Clone {}
extension Optional: Clone {}
"#;
        fs::write(sources_dir.join("RustCompat.swift"), runtime_shims)?;

        let tests_dir = output_dir
            .join("Tests")
            .join(format!("{}Tests", package_name));
        fs::create_dir_all(&tests_dir)?;

        let test_swift = format!(
            r#"import XCTest
@testable import {pkg}

final class {pkg}Tests: XCTestCase {{
    func testPackageSanity() throws {{
        XCTAssertTrue(true)
    }}
}}
"#,
            pkg = package_name
        );
        let test_file_path = tests_dir.join(format!("{}Tests.swift", package_name));
        fs::write(&test_file_path, test_swift)?;

        let manifest = PackageManifest {
            package_name: package_name.to_string(),
            swift_tools_version: "5.9".into(),
            platforms: vec![
                "macOS(.v14)".into(),
                "iOS(.v17)".into(),
                "tvOS(.v17)".into(),
                "watchOS(.v10)".into(),
                "visionOS(.v1)".into(),
            ],
            generated_files: vec![
                format!("Sources/{}/{}.swift", package_name, module.name),
                format!("Sources/{}/RustCompat.swift", package_name),
                format!("Tests/{}Tests/{}Tests.swift", package_name, package_name),
                "Package.swift".into(),
            ],
            symbols,
            source_locations_enabled: self.emit_source_locations,
            test_target_generated: true,
        };

        let manifest_json = serde_json::to_string_pretty(&manifest).unwrap();
        fs::write(output_dir.join("manifest.json"), manifest_json)?;

        Ok(manifest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_swift_ir::{ConstantDef, EnumVariant, FieldDef, Param, SourceSpan};

    #[test]
    fn test_generate_enum_and_function() {
        let mut module = Module::new("StateMachine");
        module.items.push(Item::Enum(EnumDef {
            name: "State".into(),
            variants: vec!["Idle".into(), "Active".into()],
            is_public: true,
            is_codable: false,
            doc_comments: vec![],
            span: SourceSpan {
                file: "sm.rs".into(),
                start_line: 1,
                end_line: 5,
            },
        }));

        module.items.push(Item::Function(Function {
            name: "next_state".into(),
            params: vec![Param {
                name: "s".into(),
                ty: Type::Named("State".into()),
                mutable: false,
            }],
            return_type: Type::Named("State".into()),
            body: Expr::new(
                ExprKind::Variable("State.Active".into()),
                Type::Named("State".into()),
            ),
            is_public: true,
            is_async: false,
            doc_comments: vec![],
            span: SourceSpan {
                file: "sm.rs".into(),
                start_line: 7,
                end_line: 9,
            },
        }));

        let mut gen = SwiftGenerator::new();
        let (code, symbols) = gen.generate_module(&module);
        assert!(code.contains("public enum State: String"));
        assert!(code.contains("case Idle = \"Idle\""));
        assert!(code.contains("public func next_state(_ s: State) -> State"));
        assert_eq!(symbols.len(), 2);
    }

    #[test]
    fn test_wrapping_arithmetic_emits_overflow_operators() {
        let mut module = Module::new("Math");
        module.items.push(Item::Function(Function {
            name: "add_wrap".into(),
            params: vec![
                Param {
                    name: "a".into(),
                    ty: Type::I32,
                    mutable: false,
                },
                Param {
                    name: "b".into(),
                    ty: Type::I32,
                    mutable: false,
                },
            ],
            return_type: Type::I32,
            body: Expr::new(
                ExprKind::Binary {
                    op: BinaryOp::Arithmetic {
                        op: IntegerOp::Add,
                        mode: IntegerMode::Wrapping,
                    },
                    left: Box::new(Expr::new(ExprKind::Variable("a".into()), Type::I32)),
                    right: Box::new(Expr::new(ExprKind::Variable("b".into()), Type::I32)),
                },
                Type::I32,
            ),
            is_public: true,
            is_async: false,
            doc_comments: vec![],
            span: SourceSpan {
                file: "math.rs".into(),
                start_line: 1,
                end_line: 3,
            },
        }));

        let mut gen = SwiftGenerator::new();
        let (code, _) = gen.generate_module(&module);
        assert!(code.contains("(a &+ b)"));
    }

    #[test]
    fn test_contracts_emit_precondition_assert_and_fatal_error() {
        let mut module = Module::new("Contracts");
        module.items.push(Item::Function(Function {
            name: "validate".into(),
            params: vec![Param {
                name: "x".into(),
                ty: Type::I32,
                mutable: false,
            }],
            return_type: Type::I32,
            body: Expr::new(
                ExprKind::Block {
                    stmts: vec![
                        Stmt::Assert {
                            condition: Expr::new(ExprKind::Variable("x".into()), Type::Bool),
                            message: Some("x must be valid".into()),
                        },
                        Stmt::DebugAssert {
                            condition: Expr::new(ExprKind::Variable("x".into()), Type::Bool),
                            message: None,
                        },
                        Stmt::AssertEq {
                            left: Expr::new(ExprKind::Variable("x".into()), Type::I32),
                            right: Expr::new(
                                ExprKind::Literal(Literal::Int {
                                    value: 10,
                                    ty: Type::I32,
                                }),
                                Type::I32,
                            ),
                            message: Some("equal to 10".into()),
                        },
                        Stmt::Panic {
                            message: Some("aborting".into()),
                        },
                    ],
                    expr: Some(Box::new(Expr::new(
                        ExprKind::Variable("x".into()),
                        Type::I32,
                    ))),
                },
                Type::I32,
            ),
            is_public: true,
            is_async: false,
            doc_comments: vec![],
            span: SourceSpan {
                file: "contracts.rs".into(),
                start_line: 1,
                end_line: 10,
            },
        }));

        let mut gen = SwiftGenerator::new();
        let (code, _) = gen.generate_module(&module);
        assert!(code.contains("precondition(x, \"x must be valid\")"));
        assert!(code.contains("assert(x)"));
        assert!(code.contains("precondition(x == 10, \"equal to 10\")"));
        assert!(code.contains("fatalError(\"aborting\")"));
    }

    #[test]
    fn test_generate_impl_and_payload_enum() {
        let mut module = Module::new("TypesTest");
        module.items.push(Item::Struct(StructDef {
            name: "Counter".into(),
            fields: vec![FieldDef {
                name: "count".into(),
                ty: Type::I64,
                is_public: true,
                doc_comments: vec![],
            }],
            is_public: true,
            is_codable: false,
            is_observable: false,
            is_bitflags: false,
            is_document: false,
            is_reality_component: false,
            is_fskit_volume: false,
            is_widget_timeline: false,
            doc_comments: vec![],
            span: SourceSpan {
                file: "types.rs".into(),
                start_line: 1,
                end_line: 3,
            },
        }));

        module.items.push(Item::Impl(ImplBlock {
            target_type: "Counter".into(),
            methods: vec![Method {
                name: "increment".into(),
                receiver: Some(ReceiverKind::MutRef),
                params: vec![Param {
                    name: "delta".into(),
                    ty: Type::I64,
                    mutable: false,
                }],
                return_type: Type::Unit,
                is_async: false,
                body: Expr::new(
                    ExprKind::Block {
                        stmts: vec![Stmt::Assign {
                            target: Expr::new(
                                ExprKind::FieldAccess {
                                    receiver: Box::new(Expr::new(
                                        ExprKind::Variable("self".into()),
                                        Type::Named("Counter".into()),
                                    )),
                                    field: "count".into(),
                                },
                                Type::I64,
                            ),
                            value: Expr::new(
                                ExprKind::Binary {
                                    op: BinaryOp::Arithmetic {
                                        op: IntegerOp::Add,
                                        mode: IntegerMode::Checked,
                                    },
                                    left: Box::new(Expr::new(
                                        ExprKind::FieldAccess {
                                            receiver: Box::new(Expr::new(
                                                ExprKind::Variable("self".into()),
                                                Type::Named("Counter".into()),
                                            )),
                                            field: "count".into(),
                                        },
                                        Type::I64,
                                    )),
                                    right: Box::new(Expr::new(
                                        ExprKind::Variable("delta".into()),
                                        Type::I64,
                                    )),
                                },
                                Type::I64,
                            ),
                        }],
                        expr: None,
                    },
                    Type::Unit,
                ),
                is_public: true,
                doc_comments: vec![],
                span: SourceSpan {
                    file: "types.rs".into(),
                    start_line: 5,
                    end_line: 7,
                },
            }],
            constants: vec![],
            span: SourceSpan {
                file: "types.rs".into(),
                start_line: 4,
                end_line: 8,
            },
        }));

        module.items.push(Item::Enum(EnumDef {
            name: "Action".into(),
            variants: vec![
                EnumVariant {
                    name: "Stop".into(),
                    payload: VariantPayload::Unit,
                    doc_comments: vec![],
                },
                EnumVariant {
                    name: "Move".into(),
                    payload: VariantPayload::Tuple(vec![Type::I64, Type::I64]),
                    doc_comments: vec![],
                },
            ],
            is_public: true,
            is_codable: false,
            doc_comments: vec![],
            span: SourceSpan {
                file: "types.rs".into(),
                start_line: 9,
                end_line: 12,
            },
        }));

        let mut gen = SwiftGenerator::new();
        let (code, symbols) = gen.generate_module(&module);
        assert!(code.contains("public struct Counter"));
        assert!(code.contains("extension Counter"));
        assert!(code.contains("public mutating func increment(_ delta: Int64)"));
        assert!(code.contains("public enum Action: Equatable, Hashable, Sendable, Error"));
        assert!(code.contains("case Stop"));
        assert!(code.contains("case Move(Int64, Int64)"));
        assert!(!code.contains("String, CaseIterable"));
        assert!(symbols.iter().any(|s| s.swift_name == "Counter.increment"));
    }

    #[test]
    fn test_generate_option_result_string() {
        let mut module = Module::new("DataSemantics");
        module.items.push(Item::Function(Function {
            name: "test_data".into(),
            params: vec![
                Param {
                    name: "opt".into(),
                    ty: Type::Option(Box::new(Type::I64)),
                    mutable: false,
                },
                Param {
                    name: "res".into(),
                    ty: Type::Result {
                        ok: Box::new(Type::String),
                        err: Box::new(Type::Named("ErrType".into())),
                    },
                    mutable: false,
                },
                Param {
                    name: "s".into(),
                    ty: Type::String,
                    mutable: false,
                },
            ],
            return_type: Type::String,
            body: Expr::new(
                ExprKind::Slice {
                    expr: Box::new(Expr::new(ExprKind::Variable("s".into()), Type::String)),
                    start: Some(Box::new(Expr::new(
                        ExprKind::Literal(Literal::Int {
                            value: 0,
                            ty: Type::I32,
                        }),
                        Type::I32,
                    ))),
                    end: Some(Box::new(Expr::new(
                        ExprKind::Literal(Literal::Int {
                            value: 5,
                            ty: Type::I32,
                        }),
                        Type::I32,
                    ))),
                },
                Type::String,
            ),
            is_public: true,
            is_async: false,
            doc_comments: vec![],
            span: SourceSpan {
                file: "data.rs".into(),
                start_line: 1,
                end_line: 10,
            },
        }));

        let mut gen = SwiftGenerator::new();
        let (code, _) = gen.generate_module(&module);
        assert!(code.contains("opt: Int64?"));
        assert!(code.contains("res: Result<String, ErrType>"));
        assert!(code.contains("s: String"));
        assert!(code.contains("s.byteSlice(Int(0), Int(5))"));
    }

    #[test]
    fn test_codegen_inout_and_borrow_call() {
        let mut module = Module::new("Ownership");
        module.items.push(Item::Function(Function {
            name: "add_amount".into(),
            params: vec![
                Param {
                    name: "val".into(),
                    ty: Type::Ref {
                        inner: Box::new(Type::I64),
                        mutable: true,
                    },
                    mutable: true,
                },
                Param {
                    name: "delta".into(),
                    ty: Type::I64,
                    mutable: false,
                },
            ],
            return_type: Type::Unit,
            body: Expr::new(
                ExprKind::Call {
                    func: "helper".into(),
                    args: vec![Expr::new(
                        ExprKind::Borrow {
                            expr: Box::new(Expr::new(ExprKind::Variable("val".into()), Type::I64)),
                            mutable: true,
                        },
                        Type::Ref {
                            inner: Box::new(Type::I64),
                            mutable: true,
                        },
                    )],
                },
                Type::Unit,
            ),
            is_public: true,
            is_async: false,
            doc_comments: vec![],
            span: SourceSpan {
                file: "own.rs".into(),
                start_line: 1,
                end_line: 5,
            },
        }));

        let mut gen = SwiftGenerator::new();
        let (code, _) = gen.generate_module(&module);
        assert!(code.contains("public func add_amount(_ val: inout Int64, _ delta: Int64)"));
        assert!(code.contains("helper(&val)"));
    }

    #[test]
    fn test_codegen_owned_mutable_parameter_binding() {
        let mut module = Module::new("OwnedMut");
        module.items.push(Item::Function(Function {
            name: "increment".into(),
            params: vec![Param {
                name: "val".into(),
                ty: Type::I32,
                mutable: true,
            }],
            return_type: Type::I32,
            body: Expr::new(ExprKind::Variable("val".into()), Type::I32),
            is_public: true,
            is_async: false,
            doc_comments: vec![],
            span: SourceSpan {
                file: "mut_bind.rs".into(),
                start_line: 1,
                end_line: 4,
            },
        }));

        let mut gen = SwiftGenerator::new();
        let (code, _) = gen.generate_module(&module);
        // Owned parameter must NOT have 'inout' in signature:
        assert!(code.contains("public func increment(_ val: Int32) -> Int32"));
        // Owned mutable binding must introduce a local mutable copy:
        assert!(code.contains("var val = val"));
    }

    #[test]
    fn test_codegen_for_loops_and_vec() {
        let mut module = Module::new("LoopModule");
        module.items.push(Item::Function(Function {
            name: "sum_list".into(),
            params: vec![Param {
                name: "items".into(),
                ty: Type::Array {
                    element: Box::new(Type::I32),
                    size: None,
                },
                mutable: false,
            }],
            return_type: Type::I32,
            body: Expr::new(
                ExprKind::Block {
                    stmts: vec![
                        Stmt::Let {
                            name: "total".into(),
                            mutable: true,
                            ty: Some(Type::I32),
                            init: Some(Expr::new(
                                ExprKind::Literal(Literal::Int {
                                    value: 0,
                                    ty: Type::I32,
                                }),
                                Type::I32,
                            )),
                        },
                        Stmt::Expr(Expr::new(
                            ExprKind::For {
                                pattern: Pattern::Variable("x".into()),
                                iter: Box::new(Expr::new(
                                    ExprKind::Variable("items".into()),
                                    Type::Array {
                                        element: Box::new(Type::I32),
                                        size: None,
                                    },
                                )),
                                body: Box::new(Expr::new(
                                    ExprKind::Block {
                                        stmts: vec![Stmt::Assign {
                                            target: Expr::new(
                                                ExprKind::Variable("total".into()),
                                                Type::I32,
                                            ),
                                            value: Expr::new(
                                                ExprKind::Binary {
                                                    op: BinaryOp::Arithmetic {
                                                        op: IntegerOp::Add,
                                                        mode: IntegerMode::Wrapping,
                                                    },
                                                    left: Box::new(Expr::new(
                                                        ExprKind::Variable("total".into()),
                                                        Type::I32,
                                                    )),
                                                    right: Box::new(Expr::new(
                                                        ExprKind::Variable("x".into()),
                                                        Type::I32,
                                                    )),
                                                },
                                                Type::I32,
                                            ),
                                        }],
                                        expr: None,
                                    },
                                    Type::Unit,
                                )),
                            },
                            Type::Unit,
                        )),
                    ],
                    expr: Some(Box::new(Expr::new(
                        ExprKind::Variable("total".into()),
                        Type::I32,
                    ))),
                },
                Type::I32,
            ),
            is_public: true,
            is_async: false,
            doc_comments: vec![],
            span: SourceSpan {
                file: "loop.rs".into(),
                start_line: 1,
                end_line: 10,
            },
        }));

        let mut gen = SwiftGenerator::new();
        let (code, _) = gen.generate_module(&module);
        assert!(code.contains("public func sum_list(_ items: [Int32]) -> Int32"));
        assert!(code.contains("var total: Int32 = 0"));
        assert!(code.contains("for x in items {"));
        assert!(code.contains("total = (total &+ x)"));
        assert!(code.contains("return total"));
    }

    #[test]
    fn test_codegen_source_locations_directive() {
        let mut module = Module::new("SourceMapped");
        module.items.push(Item::Function(Function {
            name: "calculate".into(),
            params: vec![],
            return_type: Type::I32,
            body: Expr::new(
                ExprKind::Block {
                    stmts: vec![],
                    expr: Some(Box::new(Expr::new(
                        ExprKind::Literal(Literal::Int {
                            value: 42,
                            ty: Type::I32,
                        }),
                        Type::I32,
                    ))),
                },
                Type::I32,
            ),
            is_public: true,
            is_async: false,
            doc_comments: vec![],
            span: SourceSpan {
                file: "math/calc.rs".into(),
                start_line: 77,
                end_line: 85,
            },
        }));

        // Default: no source locations
        let mut gen_default = SwiftGenerator::new();
        let (code_default, _) = gen_default.generate_module(&module);
        assert!(!code_default.contains("#sourceLocation"));

        // With source locations enabled
        let mut gen_mapped = SwiftGenerator::new().with_source_locations(true);
        let (code_mapped, _) = gen_mapped.generate_module(&module);
        assert!(code_mapped.contains("#sourceLocation(file: \"math/calc.rs\", line: 77)"));
        assert!(code_mapped.contains("public func calculate() -> Int32"));
        assert!(code_mapped.ends_with("#sourceLocation()\n"));
    }

    #[test]
    fn test_codegen_traits_and_protocol_extensions() {
        let mut module = Module::new("TraitsDemo");
        module.items.push(Item::Trait(TraitDef {
            name: "Describable".into(),
            associated_types: vec![],
            methods: vec![
                TraitMethod {
                    name: "describe".into(),
                    receiver: Some(ReceiverKind::Ref),
                    params: vec![],
                    return_type: Type::String,
                    is_async: false,
                    default_body: None,
                    doc_comments: vec![],
                    span: SourceSpan {
                        file: "traits.rs".into(),
                        start_line: 2,
                        end_line: 4,
                    },
                },
                TraitMethod {
                    name: "default_tag".into(),
                    receiver: Some(ReceiverKind::Ref),
                    params: vec![],
                    return_type: Type::I32,
                    is_async: false,
                    default_body: Some(Expr::new(
                        ExprKind::Literal(Literal::Int {
                            value: 42,
                            ty: Type::I32,
                        }),
                        Type::I32,
                    )),
                    doc_comments: vec![],
                    span: SourceSpan {
                        file: "traits.rs".into(),
                        start_line: 5,
                        end_line: 7,
                    },
                },
            ],
            is_public: true,
            doc_comments: vec![],
            span: SourceSpan {
                file: "traits.rs".into(),
                start_line: 1,
                end_line: 8,
            },
        }));

        module.items.push(Item::TraitImpl(TraitImpl {
            trait_name: "Describable".into(),
            target_type: "Track".into(),
            type_aliases: vec![],
            where_clause: None,
            methods: vec![Method {
                name: "describe".into(),
                receiver: Some(ReceiverKind::Ref),
                params: vec![],
                return_type: Type::String,
                is_async: false,
                body: Expr::new(
                    ExprKind::Literal(Literal::String("AudioTrack".into())),
                    Type::String,
                ),
                is_public: true,
                doc_comments: vec![],
                span: SourceSpan {
                    file: "traits.rs".into(),
                    start_line: 10,
                    end_line: 14,
                },
            }],
            span: SourceSpan {
                file: "traits.rs".into(),
                start_line: 9,
                end_line: 15,
            },
        }));

        let mut gen = SwiftGenerator::new();
        let (code, symbols) = gen.generate_module(&module);
        assert!(code.contains("public protocol Describable {"));
        assert!(code.contains("func describe() -> String"));
        assert!(code.contains("func default_tag() -> Int32"));
        assert!(code.contains("public extension Describable {"));
        assert!(code.contains("return 42"));
        assert!(code.contains("extension Track: Describable {"));
        assert!(code.contains("return \"AudioTrack\""));
        assert!(symbols
            .iter()
            .any(|s| s.swift_name == "Describable" && s.kind == "trait"));
    }

    #[test]
    fn test_emit_package_generates_spm_manifest_and_tests() {
        let module = Module::new("CoreLogic");
        let temp_dir = std::env::temp_dir().join("test_emit_package_spm");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let mut gen = SwiftGenerator::new();
        let manifest = gen.emit_package(&module, &temp_dir, "CoreLogic").unwrap();

        assert_eq!(manifest.package_name, "CoreLogic");
        assert!(manifest.test_target_generated);
        assert!(manifest.platforms.iter().any(|p| p.contains("macOS(.v14)")));
        assert!(manifest.platforms.iter().any(|p| p.contains("iOS(.v17)")));

        let package_swift = fs::read_to_string(temp_dir.join("Package.swift")).unwrap();
        assert!(package_swift.contains(".macOS(.v14)"));
        assert!(package_swift.contains(".iOS(.v17)"));
        assert!(package_swift.contains(".testTarget("));
        assert!(package_swift.contains("name: \"CoreLogicTests\""));

        let test_file = temp_dir.join("Tests/CoreLogicTests/CoreLogicTests.swift");
        assert!(test_file.exists());
        let test_content = fs::read_to_string(&test_file).unwrap();
        assert!(test_content.contains("import XCTest"));
        assert!(test_content.contains("@testable import CoreLogic"));
    }

    #[test]
    fn test_codegen_codable_struct_and_enum() {
        let mut module = Module::new("CodableTest");
        module.items.push(Item::Struct(StructDef {
            name: "Payload".into(),
            fields: vec![FieldDef {
                name: "data".into(),
                ty: Type::String,
                is_public: true,
                doc_comments: vec![],
            }],
            is_public: true,
            is_codable: true,
            is_observable: false,
            is_bitflags: false,
            is_document: false,
            is_reality_component: false,
            is_fskit_volume: false,
            is_widget_timeline: false,
            doc_comments: vec![],
            span: SourceSpan::default(),
        }));
        module.items.push(Item::Enum(EnumDef {
            name: "Command".into(),
            variants: vec![
                EnumVariant {
                    name: "Halt".into(),
                    payload: VariantPayload::Unit,
                    doc_comments: vec![],
                },
                EnumVariant {
                    name: "Send".into(),
                    payload: VariantPayload::Tuple(vec![Type::String]),
                    doc_comments: vec![],
                },
            ],
            is_public: true,
            is_codable: true,
            doc_comments: vec![],
            span: SourceSpan::default(),
        }));

        let mut gen = SwiftGenerator::new();
        let (code, _) = gen.generate_module(&module);
        assert!(code.contains("public struct Payload: Equatable, Hashable, Codable, Sendable {"));
        assert!(
            code.contains("public enum Command: Equatable, Hashable, Codable, Sendable, Error {")
        );
    }

    #[test]
    fn test_codegen_observable_class() {
        let mut module = Module::new("ObservableTest");
        module.items.push(Item::Struct(StructDef {
            name: "ItemState".into(),
            fields: vec![
                FieldDef {
                    name: "id".into(),
                    ty: Type::I64,
                    is_public: true,
                    doc_comments: vec![],
                },
                FieldDef {
                    name: "count".into(),
                    ty: Type::I64,
                    is_public: true,
                    doc_comments: vec![],
                },
            ],
            is_public: true,
            is_codable: false,
            is_observable: true,
            is_bitflags: false,
            is_document: false,
            is_reality_component: false,
            is_fskit_volume: false,
            is_widget_timeline: false,
            doc_comments: vec![],
            span: SourceSpan::default(),
        }));
        module.items.push(Item::Impl(ImplBlock {
            target_type: "ItemState".into(),
            methods: vec![Method {
                name: "increment".into(),
                receiver: Some(ReceiverKind::MutRef),
                params: vec![],
                return_type: Type::Unit,
                is_async: false,
                body: Expr::new(
                    ExprKind::Block {
                        stmts: vec![Stmt::Assign {
                            target: Expr::new(
                                ExprKind::FieldAccess {
                                    receiver: Box::new(Expr::new(
                                        ExprKind::Variable("self".into()),
                                        Type::Named("ItemState".into()),
                                    )),
                                    field: "count".into(),
                                },
                                Type::I64,
                            ),
                            value: Expr::new(
                                ExprKind::Binary {
                                    op: BinaryOp::Arithmetic {
                                        op: IntegerOp::Add,
                                        mode: IntegerMode::Wrapping,
                                    },
                                    left: Box::new(Expr::new(
                                        ExprKind::FieldAccess {
                                            receiver: Box::new(Expr::new(
                                                ExprKind::Variable("self".into()),
                                                Type::Named("ItemState".into()),
                                            )),
                                            field: "count".into(),
                                        },
                                        Type::I64,
                                    )),
                                    right: Box::new(Expr::new(
                                        ExprKind::Literal(Literal::Int {
                                            value: 1,
                                            ty: Type::I64,
                                        }),
                                        Type::I64,
                                    )),
                                },
                                Type::I64,
                            ),
                        }],
                        expr: None,
                    },
                    Type::Unit,
                ),
                is_public: true,
                doc_comments: vec![],
                span: SourceSpan::default(),
            }],
            constants: vec![],
            span: SourceSpan::default(),
        }));

        let mut gen = SwiftGenerator::new();
        let (code, _) = gen.generate_module(&module);
        assert!(code.contains("import Observation"));
        assert!(code.contains("@Observable\npublic final class ItemState: Identifiable {"));
        assert!(code.contains("public func increment() {"));
        assert!(!code.contains("public mutating func increment()"));
    }

    #[test]
    fn test_codegen_swiftui_view() {
        let mut module = Module::new("ui_test");
        let view = ViewDef {
            name: "CounterView".into(),
            properties: vec![
                rust_swift_ir::ViewProperty {
                    name: "model".into(),
                    ty: Type::Named("CounterViewModel".into()),
                    kind: PropertyBindingKind::Observable,
                    is_public: true,
                },
                rust_swift_ir::ViewProperty {
                    name: "title".into(),
                    ty: Type::String,
                    kind: PropertyBindingKind::Plain,
                    is_public: true,
                },
            ],
            root: ViewNode {
                kind: ViewKind::VStack { spacing: Some(8) },
                children: vec![
                    ViewNode {
                        kind: ViewKind::Text("title".into()),
                        children: Vec::new(),
                        modifiers: vec![
                            ViewModifier::Font("headline".into()),
                            ViewModifier::Padding(None),
                        ],
                        span: SourceSpan::default(),
                    },
                    ViewNode {
                        kind: ViewKind::HStack { spacing: None },
                        children: vec![
                            ViewNode {
                                kind: ViewKind::Button {
                                    label: "Decrement".into(),
                                    action: "model.decrement()".into(),
                                },
                                children: Vec::new(),
                                modifiers: Vec::new(),
                                span: SourceSpan::default(),
                            },
                            ViewNode {
                                kind: ViewKind::Spacer,
                                children: Vec::new(),
                                modifiers: Vec::new(),
                                span: SourceSpan::default(),
                            },
                            ViewNode {
                                kind: ViewKind::Button {
                                    label: "Increment".into(),
                                    action: "model.increment()".into(),
                                },
                                children: Vec::new(),
                                modifiers: Vec::new(),
                                span: SourceSpan::default(),
                            },
                        ],
                        modifiers: Vec::new(),
                        span: SourceSpan::default(),
                    },
                ],
                modifiers: vec![ViewModifier::Padding(Some(16))],
                span: SourceSpan::default(),
            },
            is_public: true,
            span: SourceSpan::default(),
        };

        module.items.push(Item::View(view));

        let mut gen = SwiftGenerator::new();
        let (code, _) = gen.generate_module(&module);
        assert!(code.contains("import SwiftUI"));
        assert!(code.contains("import Observation"));
        assert!(code.contains("public struct CounterView: View {"));
        assert!(code.contains("@Bindable public var model: CounterViewModel"));
        assert!(code.contains("public var title: String"));
        assert!(code.contains("public var body: some View {"));
        assert!(code.contains("VStack(spacing: 8) {"));
        assert!(code.contains("Text(\"title\")"));
        assert!(code.contains(".font(.headline)"));
        assert!(code.contains(".padding()"));
        assert!(code.contains("HStack {"));
        assert!(code.contains("Button(\"Decrement\") {"));
        assert!(code.contains("model.decrement()"));
        assert!(code.contains("Spacer()"));
    }

    #[test]
    fn test_codegen_bitflags_optionset() {
        let mut module = Module::new("BitflagsTest");
        module.items.push(Item::Struct(StructDef {
            name: "FilePermissions".into(),
            fields: vec![FieldDef {
                name: "raw_value".into(),
                ty: Type::U32,
                is_public: true,
                doc_comments: vec![],
            }],
            is_public: true,
            is_codable: false,
            is_observable: false,
            is_bitflags: true,
            is_document: false,
            is_reality_component: false,
            is_fskit_volume: false,
            is_widget_timeline: false,
            doc_comments: vec![],
            span: SourceSpan::default(),
        }));

        module.items.push(Item::Impl(ImplBlock {
            target_type: "FilePermissions".into(),
            methods: vec![],
            constants: vec![
                ConstantDef {
                    name: "READ".into(),
                    ty: Type::Named("FilePermissions".into()),
                    value: Expr::new(
                        ExprKind::StructInit {
                            name: "FilePermissions".into(),
                            fields: vec![(
                                "raw_value".into(),
                                Expr::new(
                                    ExprKind::Literal(Literal::Int {
                                        value: 1,
                                        ty: Type::U32,
                                    }),
                                    Type::U32,
                                ),
                            )],
                        },
                        Type::Named("FilePermissions".into()),
                    ),
                    is_public: true,
                    doc_comments: vec![],
                    span: SourceSpan::default(),
                },
                ConstantDef {
                    name: "WRITE".into(),
                    ty: Type::Named("FilePermissions".into()),
                    value: Expr::new(
                        ExprKind::StructInit {
                            name: "FilePermissions".into(),
                            fields: vec![(
                                "raw_value".into(),
                                Expr::new(
                                    ExprKind::Literal(Literal::Int {
                                        value: 2,
                                        ty: Type::U32,
                                    }),
                                    Type::U32,
                                ),
                            )],
                        },
                        Type::Named("FilePermissions".into()),
                    ),
                    is_public: true,
                    doc_comments: vec![],
                    span: SourceSpan::default(),
                },
            ],
            span: SourceSpan::default(),
        }));

        let mut gen = SwiftGenerator::new();
        let (code, _) = gen.generate_module(&module);
        assert!(code.contains("public struct FilePermissions: OptionSet, Sendable {"));
        assert!(code.contains("public let rawValue: UInt32"));
        assert!(code.contains("public init(rawValue: UInt32) {"));
        assert!(code.contains("self.rawValue = rawValue"));
        assert!(code.contains("public static func | (lhs: FilePermissions, rhs: FilePermissions) -> FilePermissions {"));
        assert!(code.contains("public static func & (lhs: FilePermissions, rhs: FilePermissions) -> FilePermissions {"));
        assert!(code.contains("extension FilePermissions {"));
        assert!(
            code.contains("public static let READ: FilePermissions = FilePermissions(rawValue: 1)")
        );
        assert!(code
            .contains("public static let WRITE: FilePermissions = FilePermissions(rawValue: 2)"));
    }

    #[test]
    fn test_codegen_file_scope_constants_and_type_aliases() {
        let mut module = Module::new("ConstantsDemo");
        module.items.push(Item::Constant(ConstantDef {
            name: "MAX_RETRIES".into(),
            ty: Type::U32,
            value: Expr::new(
                ExprKind::Literal(Literal::Int {
                    value: 5,
                    ty: Type::U32,
                }),
                Type::U32,
            ),
            is_public: true,
            doc_comments: vec!["Maximum retry count.".into()],
            span: SourceSpan::default(),
        }));

        module.items.push(Item::TypeAlias(TypeAlias {
            name: "Identifier".into(),
            target: Type::U64,
            is_public: true,
            doc_comments: vec!["Unique user identifier.".into()],
            span: SourceSpan::default(),
        }));

        let mut gen = SwiftGenerator::new();
        let (code, symbols) = gen.generate_module(&module);
        assert!(code.contains("/// Maximum retry count."));
        assert!(code.contains("public let MAX_RETRIES: UInt32 = 5"));
        assert!(code.contains("/// Unique user identifier."));
        assert!(code.contains("public typealias Identifier = UInt64"));
        assert!(symbols.iter().any(|s| s.swift_name == "MAX_RETRIES"));
        assert!(symbols.iter().any(|s| s.swift_name == "Identifier"));
    }

    #[test]
    fn test_codegen_string_format_interpolation() {
        let mut module = Module::new("FormatDemo");
        module.items.push(Item::Function(Function {
            name: "greet".into(),
            params: vec![
                Param {
                    name: "name".into(),
                    ty: Type::String,
                    mutable: false,
                },
                Param {
                    name: "count".into(),
                    ty: Type::I32,
                    mutable: false,
                },
            ],
            return_type: Type::String,
            body: Expr::new(
                ExprKind::StringFormat {
                    template: "Hello {}, you have {} messages!".into(),
                    args: vec![
                        Expr::new(ExprKind::Variable("name".into()), Type::String),
                        Expr::new(ExprKind::Variable("count".into()), Type::I32),
                    ],
                },
                Type::String,
            ),
            is_public: true,
            is_async: false,
            doc_comments: vec!["Greets a user with unread count.".into()],
            span: SourceSpan::default(),
        }));

        let mut gen = SwiftGenerator::new();
        let (code, _) = gen.generate_module(&module);
        assert!(code.contains("/// Greets a user with unread count."));
        assert!(code.contains("return \"Hello \\(name), you have \\(count) messages!\""));
    }

    #[test]
    fn test_codegen_emits_doc_comments_on_structs_and_fields() {
        let mut module = Module::new("DocTest");
        module.items.push(Item::Struct(StructDef {
            name: "Config".into(),
            fields: vec![FieldDef {
                name: "timeout_ms".into(),
                ty: Type::U64,
                is_public: true,
                doc_comments: vec!["Timeout in milliseconds.".into()],
            }],
            is_public: true,
            is_codable: false,
            is_observable: false,
            is_bitflags: false,
            is_document: false,
            is_reality_component: false,
            is_fskit_volume: false,
            is_widget_timeline: false,
            doc_comments: vec!["Main configuration payload.".into()],
            span: SourceSpan::default(),
        }));

        let mut gen = SwiftGenerator::new();
        let (code, _) = gen.generate_module(&module);
        assert!(code.contains("/// Main configuration payload."));
        assert!(code.contains("public struct Config: Equatable, Hashable, Sendable {"));
        assert!(code.contains("    /// Timeout in milliseconds."));
        assert!(code.contains("    public var timeout_ms: UInt64"));
    }

    #[test]
    fn test_codegen_associated_types_and_where_clause() {
        let mut module = Module::new("AssocTypesDemo");
        module.items.push(Item::Trait(TraitDef {
            name: "Container".into(),
            associated_types: vec![rust_swift_ir::AssociatedTypeDef {
                name: "Item".into(),
                bounds: vec!["Equatable".into(), "Sendable".into()],
                span: SourceSpan::default(),
            }],
            methods: vec![],
            is_public: true,
            doc_comments: vec![],
            span: SourceSpan::default(),
        }));

        module.items.push(Item::TraitImpl(TraitImpl {
            trait_name: "Container".into(),
            target_type: "MyBox".into(),
            type_aliases: vec![TypeAlias {
                name: "Item".into(),
                target: Type::I64,
                is_public: true,
                doc_comments: vec![],
                span: SourceSpan::default(),
            }],
            where_clause: Some("T: Sendable".into()),
            methods: vec![],
            span: SourceSpan::default(),
        }));

        let mut gen = SwiftGenerator::new();
        let (code, _) = gen.generate_module(&module);
        assert!(code.contains("associatedtype Item: Equatable, Sendable"));
        assert!(code.contains("extension MyBox: Container where T: Sendable {"));
        assert!(code.contains("public typealias Item = Int64"));
    }

    #[test]
    fn test_codegen_async_function_and_await() {
        let mut module = Module::new("AsyncDemo");
        module.items.push(Item::Function(Function {
            name: "fetch_count".into(),
            params: vec![],
            return_type: Type::I64,
            is_public: true,
            is_async: true,
            doc_comments: vec![],
            span: SourceSpan::default(),
            body: Expr::new(
                ExprKind::Block {
                    stmts: vec![],
                    expr: Some(Box::new(Expr::new(
                        ExprKind::Await(Box::new(Expr::new(
                            ExprKind::Call {
                                func: "get_remote".into(),
                                args: vec![],
                            },
                            Type::I64,
                        ))),
                        Type::I64,
                    ))),
                },
                Type::I64,
            ),
        }));

        let mut gen = SwiftGenerator::new();
        let (code, _) = gen.generate_module(&module);
        assert!(code.contains("public func fetch_count() async -> Int64 {"));
        assert!(code.contains("return await get_remote()"));
    }

    #[test]
    fn test_codegen_apple_ecosystem_kits() {
        let mut module = Module::new("AppleKitsDemo");
        module.items.push(Item::Struct(StructDef {
            name: "TextDocument".into(),
            fields: vec![FieldDef {
                name: "text".into(),
                ty: Type::String,
                is_public: true,
                doc_comments: vec![],
            }],
            is_public: true,
            is_codable: true,
            is_observable: false,
            is_bitflags: false,
            is_document: true,
            is_reality_component: false,
            is_fskit_volume: false,
            is_widget_timeline: false,
            doc_comments: vec![],
            span: SourceSpan::default(),
        }));

        module.items.push(Item::Struct(StructDef {
            name: "PhysicsComponent".into(),
            fields: vec![FieldDef {
                name: "velocity".into(),
                ty: Type::I64,
                is_public: true,
                doc_comments: vec![],
            }],
            is_public: true,
            is_codable: true,
            is_observable: false,
            is_bitflags: false,
            is_document: false,
            is_reality_component: true,
            is_fskit_volume: false,
            is_widget_timeline: false,
            doc_comments: vec![],
            span: SourceSpan::default(),
        }));

        module.items.push(Item::Struct(StructDef {
            name: "MyDisk".into(),
            fields: vec![FieldDef {
                name: "sectors".into(),
                ty: Type::I64,
                is_public: true,
                doc_comments: vec![],
            }],
            is_public: true,
            is_codable: false,
            is_observable: false,
            is_bitflags: false,
            is_document: false,
            is_reality_component: false,
            is_fskit_volume: true,
            is_widget_timeline: false,
            doc_comments: vec![],
            span: SourceSpan::default(),
        }));

        module.items.push(Item::Struct(StructDef {
            name: "ClockWidget".into(),
            fields: vec![],
            is_public: true,
            is_codable: false,
            is_observable: false,
            is_bitflags: false,
            is_document: false,
            is_reality_component: false,
            is_fskit_volume: false,
            is_widget_timeline: true,
            doc_comments: vec![],
            span: SourceSpan::default(),
        }));

        let mut gen = SwiftGenerator::new();
        let (code, _) = gen.generate_module(&module);
        assert!(code.contains("import SwiftUI"));
        assert!(code.contains("import UniformTypeIdentifiers"));
        assert!(code.contains("import RealityKit"));
        assert!(code.contains("import FSKit"));
        assert!(code.contains("import WidgetKit"));

        // FileDocument conformance:
        assert!(code.contains("extension TextDocument: FileDocument {"));
        assert!(code.contains("public static var readableContentTypes: [UTType] { [.json] }"));

        // RealityKit Component conformance:
        assert!(code.contains("public struct PhysicsComponent: Equatable, Hashable, Codable, Sendable, RealityKit.Component {"));

        // FSKit Volume:
        assert!(code.contains("public final class MyDiskVolume: FSVolume, FSVolumeOperations {"));

        // WidgetKit TimelineProvider:
        assert!(code.contains("extension ClockWidget: TimelineProvider {"));
    }
}
