# rust-swift-ir

Target-neutral Semantic Intermediate Representation (IR) and Compiler Semantic Graph (CSG) bridge.

## Overview

`rust-swift-ir` defines the typed intermediate representation between the Rust frontend and the Swift code generator. It also provides the projection engine mapping modules, items, views, and contracts into standard Compiler Semantic Graph (CSG) nodes and edges conforming to the **Reflective Rust** (`reflective-rust-csg`) specifications.

## Core Abstractions

- **`Module`**: Top-level compilation unit containing type definitions, functions, traits, view hierarchies, and CSG graph provenance.
- **`Type`**: Supported primitives (`Int`, `Bool`, `String`), dynamic `Vec<T>`, tuples, `Option<T>`, `Result<T, E>`, and custom user types.
- **`StructDef` & `EnumDef`**: Complete structural metadata including `#[derive(Codable)]` flags, `@Observable` classification, and payload variant associated values.
- **`ViewDef` & `ViewNode`**: Declarative UI tree representation supporting containers (`VStack`, `HStack`, `ZStack`, `List`), leaf elements (`Text`, `Button`, `TextField`, `Spacer`, `Divider`), and modifiers (`padding`, `font`, `foregroundColor`, `background`, `frame`).
- **`Stmt` & `Expr`**: Pure expressions, arithmetic operations (with explicit `Wrapping` / `Checked` modes), sequence loops, variable bindings, and contract assertions (`precondition`, `assert`, `fatalError`).
- **`CSGProjection`**: Builds directed semantic dependency graphs with typed nodes (`Module`, `Function`, `Type`, `View`, `Contract`) and edges (`Defines`, `Calls`, `Binds`, `Enforces`).
