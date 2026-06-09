# Trait Contracts: Rule, RuleVisitor, and AnalyzeContext

**Phase:** M0.5 (Locked — final before M1/M2 implementation)  
**Date:** 2026-06-09  
**Audience:** Rule engineers implementing 60+ lint rules in M2 and beyond  
**Stability:** ✅ **LOCKED** — no changes without Architect consensus

---

## Overview

This document defines the **exact Rust signatures and contracts** for:
1. **`Rule` trait** — entry point for lint rules; immutable, thread-safe
2. **`RuleVisitor` trait** — visitor pattern for AST traversal; rule-specific implementations
3. **`AnalyzeContext`** — per-file context passed to every rule

These traits are the **contract layer** between the parsing and diagnostic phases. Rules are **immutable**, **thread-safe**, and **executable in parallel per file** via Rayon. A rule engineer reading this document should be able to implement any of the 60 lint rules without needing clarification on trait semantics or architecture.

---

## 1. Rule Trait

### 1.1 Signature

```rust
/// Trait implemented by every lint rule.
///
/// Rule instances are **immutable** for thread safety and Rayon parallelism.
/// Each rule performs analysis on a single Dart file (Program) and returns
/// diagnostics. Multiple rules analyze the same file in parallel.
pub trait Rule: Send + Sync {
    /// Returns the canonical rule identifier (snake_case).
    ///
    /// Example: "avoid_dynamic", "no_empty_block", "prefer_trailing_comma".
    /// Must be unique per rule registry.
    fn name(&self) -> &'static str;

    /// Analyzes a single Dart file and returns all diagnostics for this rule.
    ///
    /// # Arguments
    /// - `program`: Fully parsed Dart compilation unit (Program::declarations)
    /// - `ctx`: Per-file analysis context (source, file_path, config)
    ///
    /// # Returns
    /// A Vec of Diagnostic structs. Empty if no violations found.
    ///
    /// # Thread Safety
    /// MUST NOT use mutable self state. All state required for analysis
    /// must be captured at construction time (e.g., threshold, exclude_list).
    /// This enables Rayon to parallelize analyze() across files safely.
    ///
    /// # Example
    /// ```ignore
    /// fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
    ///     let mut diagnostics = vec![];
    ///     for decl in &program.declarations {
    ///         // Inspect decl, accumulate diagnostics
    ///         if /* violation */ {
    ///             diagnostics.push(Diagnostic {
    ///                 rule: self.name(),
    ///                 severity: Severity::Warning,
    ///                 message: "...".to_string(),
    ///                 file_path: ctx.file_path.to_string_lossy().to_string(),
    ///                 span: Span { start: X, end: Y },
    ///             });
    ///         }
    ///     }
    ///     diagnostics
    /// }
    /// ```
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic>;
}
```

### 1.2 Immutability Requirement

**Why immutable self?**

- **Rayon parallelism**: Each file is analyzed in parallel by ALL rules. Mutable self would require `Arc<Mutex<Rule>>`, which serializes across files — defeating parallelism.
- **Correctness**: No two threads can access the same rule's mutable state simultaneously. Immutable `&self` eliminates data races by construction.

**How to carry configuration?**

Configuration is part of the rule struct, captured at construction:

```rust
/// Example: avoid_dynamic rule with no configuration.
pub struct AvoidDynamic;

impl Rule for AvoidDynamic {
    fn name(&self) -> &'static str { "avoid_dynamic" }
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> { /* ... */ }
}

/// Example: no_magic_number rule with threshold config.
pub struct NoMagicNumber {
    threshold: usize,  // Configured at construction
    exclude: Vec<u64>, // Configured at construction
}

impl NoMagicNumber {
    pub fn new(threshold: usize, exclude: Vec<u64>) -> Self {
        Self { threshold, exclude }
    }
}

impl Rule for NoMagicNumber {
    fn name(&self) -> &'static str { "no_magic_number" }
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        // Use self.threshold and self.exclude (read-only)
        // ...
    }
}
```

### 1.3 Error Handling

Rules **do not propagate errors** from `analyze()`. If parsing or analysis fails:

1. Log the error to stderr (via `eprintln!` or tracing crate — implemented in M2)
2. Return an empty `Vec<Diagnostic>` (conservative — no false positives)
3. Continue analyzing other files

Example:

```rust
fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
    let mut diagnostics = vec![];
    
    for decl in &program.declarations {
        match self.check_declaration(decl) {
            Ok(diags) => diagnostics.extend(diags),
            Err(e) => {
                eprintln!("Rule {} error in {}: {}", self.name(), ctx.file_path.display(), e);
                // Return what we have; don't propagate error
            }
        }
    }
    
    diagnostics
}
```

---

## 2. RuleVisitor Trait

The `RuleVisitor` trait provides a **default visitor pattern** for AST traversal. Rules may optionally implement it to structure analysis as visitors instead of direct traversal.

**Status:** To be defined in M1 alongside AST node types. Stubbed here for reference.

### 2.1 Visitor Contract (Provisional)

```rust
/// Visitor pattern over Dart AST nodes.
///
/// Default implementations return empty Vec<Diagnostic>; rules override
/// only the node types they care about.
///
/// RULE: Each visit_X() method MUST be immutable (&self).
/// No mutable self tracking is allowed.
pub trait RuleVisitor: Rule {
    // Class and type declarations
    fn visit_class_declaration(&self, node: &ClassDeclaration, ctx: &AnalyzeContext) -> Vec<Diagnostic> { vec![] }
    fn visit_mixin_declaration(&self, node: &MixinDeclaration, ctx: &AnalyzeContext) -> Vec<Diagnostic> { vec![] }
    fn visit_extension_declaration(&self, node: &ExtensionDeclaration, ctx: &AnalyzeContext) -> Vec<Diagnostic> { vec![] }
    fn visit_enum_declaration(&self, node: &EnumDeclaration, ctx: &AnalyzeContext) -> Vec<Diagnostic> { vec![] }
    fn visit_typedef_declaration(&self, node: &TypedefDeclaration, ctx: &AnalyzeContext) -> Vec<Diagnostic> { vec![] }

    // Functions and methods
    fn visit_function_declaration(&self, node: &FunctionDeclaration, ctx: &AnalyzeContext) -> Vec<Diagnostic> { vec![] }
    fn visit_method_declaration(&self, node: &MethodDeclaration, ctx: &AnalyzeContext) -> Vec<Diagnostic> { vec![] }
    fn visit_constructor_declaration(&self, node: &ConstructorDeclaration, ctx: &AnalyzeContext) -> Vec<Diagnostic> { vec![] }

    // Statements
    fn visit_block_statement(&self, node: &BlockStatement, ctx: &AnalyzeContext) -> Vec<Diagnostic> { vec![] }
    fn visit_if_statement(&self, node: &IfStatement, ctx: &AnalyzeContext) -> Vec<Diagnostic> { vec![] }
    fn visit_for_statement(&self, node: &ForStatement, ctx: &AnalyzeContext) -> Vec<Diagnostic> { vec![] }
    fn visit_while_statement(&self, node: &WhileStatement, ctx: &AnalyzeContext) -> Vec<Diagnostic> { vec![] }
    fn visit_try_statement(&self, node: &TryStatement, ctx: &AnalyzeContext) -> Vec<Diagnostic> { vec![] }
    fn visit_switch_statement(&self, node: &SwitchStatement, ctx: &AnalyzeContext) -> Vec<Diagnostic> { vec![] }
    fn visit_expression_statement(&self, node: &ExpressionStatement, ctx: &AnalyzeContext) -> Vec<Diagnostic> { vec![] }
    fn visit_return_statement(&self, node: &ReturnStatement, ctx: &AnalyzeContext) -> Vec<Diagnostic> { vec![] }
    fn visit_throw_statement(&self, node: &ThrowStatement, ctx: &AnalyzeContext) -> Vec<Diagnostic> { vec![] }

    // Expressions
    fn visit_binary_expression(&self, node: &BinaryExpression, ctx: &AnalyzeContext) -> Vec<Diagnostic> { vec![] }
    fn visit_unary_expression(&self, node: &UnaryExpression, ctx: &AnalyzeContext) -> Vec<Diagnostic> { vec![] }
    fn visit_call_expression(&self, node: &CallExpression, ctx: &AnalyzeContext) -> Vec<Diagnostic> { vec![] }
    fn visit_member_access(&self, node: &MemberAccess, ctx: &AnalyzeContext) -> Vec<Diagnostic> { vec![] }
    fn visit_literal_expression(&self, node: &LiteralExpression, ctx: &AnalyzeContext) -> Vec<Diagnostic> { vec![] }
    fn visit_identifier(&self, node: &Identifier, ctx: &AnalyzeContext) -> Vec<Diagnostic> { vec![] }

    // Type annotations
    fn visit_type_annotation(&self, node: &TypeAnnotation, ctx: &AnalyzeContext) -> Vec<Diagnostic> { vec![] }
    fn visit_formal_parameter(&self, node: &FormalParameter, ctx: &AnalyzeContext) -> Vec<Diagnostic> { vec![] }

    // Variables
    fn visit_variable_declaration(&self, node: &VariableDeclaration, ctx: &AnalyzeContext) -> Vec<Diagnostic> { vec![] }

    // Imports
    fn visit_import_directive(&self, node: &ImportDirective, ctx: &AnalyzeContext) -> Vec<Diagnostic> { vec![] }
    fn visit_export_directive(&self, node: &ExportDirective, ctx: &AnalyzeContext) -> Vec<Diagnostic> { vec![] }

    // Comments (if tracked)
    fn visit_comment(&self, node: &Comment, ctx: &AnalyzeContext) -> Vec<Diagnostic> { vec![] }
}
```

### 2.2 How Rule::analyze() Uses RuleVisitor

Typically, a rule that implements both `Rule` and `RuleVisitor` drives the visitor in `analyze()`:

```rust
pub struct AvoidDynamic;

impl Rule for AvoidDynamic {
    fn name(&self) -> &'static str { "avoid_dynamic" }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diagnostics = vec![];
        
        // Traverse all declarations
        for decl in &program.declarations {
            // Call visitor method for each declaration
            diagnostics.extend(self.visit_declaration(decl, ctx));
        }
        
        diagnostics
    }
}

impl RuleVisitor for AvoidDynamic {
    fn visit_type_annotation(&self, node: &TypeAnnotation, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diagnostics = vec![];
        
        if node.is_dynamic() {
            diagnostics.push(Diagnostic {
                rule: self.name(),
                severity: Severity::Warning,
                message: "Avoid using 'dynamic'. Use a specific type instead.".to_string(),
                file_path: ctx.file_path.to_string_lossy().to_string(),
                span: node.span(),
            });
        }
        
        diagnostics
    }
}
```

### 2.3 Helper: Recursive Traversal

A rule may implement a private recursive traversal helper to visit all nodes:

```rust
impl AvoidDynamic {
    fn visit_declaration(&self, decl: &Declaration, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = vec![];
        
        match decl {
            Declaration::Class(class) => {
                // Visit class members recursively
                for member in &class.members {
                    diags.extend(self.visit_class_member(member, ctx));
                }
            }
            Declaration::Function(func) => {
                diags.extend(self.visit_type_annotation(&func.return_type, ctx));
                for param in &func.parameters {
                    diags.extend(self.visit_type_annotation(&param.type_annotation, ctx));
                }
            }
            // ... other Declaration variants
            _ => {}
        }
        
        diags
    }

    fn visit_class_member(&self, member: &ClassMember, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = vec![];
        
        match member {
            ClassMember::Field(field) => {
                diags.extend(self.visit_type_annotation(&field.type_annotation, ctx));
            }
            ClassMember::Method(method) => {
                diags.extend(self.visit_type_annotation(&method.return_type, ctx));
                for param in &method.parameters {
                    diags.extend(self.visit_type_annotation(&param.type_annotation, ctx));
                }
            }
            // ... etc
            _ => {}
        }
        
        diags
    }
}
```

---

## 3. AnalyzeContext

### 3.1 Structure

```rust
/// Per-file analysis context passed to every rule.
///
/// AnalyzeContext provides read-only access to the source, file path, and config.
/// All fields are immutable (borrowed references).
pub struct AnalyzeContext<'a> {
    /// Path to the .dart file being analyzed (relative or absolute).
    /// Used for diagnostic reporting and rule-specific filtering (e.g., exclude_test_files).
    pub file_path: &'a std::path::Path,

    /// Raw Dart source code for the file.
    /// Used for span-based source lookup and error context generation.
    pub source: &'a str,

    /// Loaded jdlint.json configuration.
    /// Use for rule-specific overrides (thresholds, exclude lists, etc.).
    pub config: &'a JdlintConfig,
}
```

### 3.2 Lifetime

`AnalyzeContext<'a>` uses a borrowed lifetime `'a` to avoid cloning:
- `file_path` references a Path from the file work unit
- `source` references the entire file's source string
- `config` references the global config

Rules **do not store** `AnalyzeContext`; they only use it during `analyze()` call.

### 3.3 Creating Diagnostics

To emit a diagnostic, construct a `Diagnostic` struct:

```rust
use jdlint_diagnostics::{Diagnostic, Severity, Span};

fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
    let mut diagnostics = vec![];

    // ... analysis logic ...

    if /* violation */ {
        diagnostics.push(Diagnostic {
            rule: self.name(),                         // &'static str
            severity: Severity::Warning,                // or Error, Info
            message: "Violation description".to_string(),
            file_path: ctx.file_path.to_string_lossy().to_string(), // Convert Path → String
            span: Span {
                start: start_byte_offset,               // Byte offset in source
                end: end_byte_offset,                   // Byte offset in source
            },
        });
    }

    diagnostics
}
```

### 3.4 Span Calculation from AST Nodes

AST nodes (M1) will provide a `.span()` method or field that returns a `Span` directly:

```rust
if node.is_dynamic() {
    diagnostics.push(Diagnostic {
        rule: self.name(),
        severity: Severity::Warning,
        message: "Avoid using 'dynamic'".to_string(),
        file_path: ctx.file_path.to_string_lossy().to_string(),
        span: node.span(), // ← AST node provides span directly
    });
}
```

If manual span calculation is needed, use byte offsets into `ctx.source`:

```rust
// Find "dynamic" keyword in source; calculate span
let start = ctx.source.find("dynamic").unwrap_or(0);
let end = start + "dynamic".len();
let span = Span { start, end };
```

### 3.5 File Path Filtering

Some rules skip certain files (e.g., test files, generated code):

```rust
impl Rule for MyRule {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        // Skip .g.dart (generated) and *_test.dart files
        if ctx.file_path.to_string_lossy().ends_with(".g.dart")
            || ctx.file_path.to_string_lossy().ends_with("_test.dart") {
            return vec![];
        }

        // ... analyze ...
    }
}
```

### 3.6 Config Access

Rules access rule-specific config via `ctx.config`:

```rust
impl Rule for NoMagicNumber {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        // Get rule config (M3 — will be added to JdlintConfig)
        // let rule_config = ctx.config.rules.no_magic_number;
        // let threshold = rule_config.threshold;
        
        // For now (M2), use struct field set at construction
        // ...
    }
}
```

---

## 4. End-to-End Example: `avoid_dynamic` Rule

This example demonstrates a complete, compilable rule implementation using stub AST types (marked `// TODO: defined in M1`).

### 4.1 Full Implementation

```rust
// File: crates/jdlint_rules/src/avoid_dynamic.rs

use jdlint_analyze::{Rule, RuleVisitor, AnalyzeContext};
use jdlint_diagnostics::{Diagnostic, Severity, Span};
use jdlint_syntax::Program;

/// Rule: avoid_dynamic
/// Flag any use of `dynamic` as a type annotation.
#[derive(Debug, Clone)]
pub struct AvoidDynamic;

impl Rule for AvoidDynamic {
    fn name(&self) -> &'static str {
        "avoid_dynamic"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diagnostics = vec![];

        // Traverse all top-level declarations
        for decl in &program.declarations {
            diagnostics.extend(self.visit_declaration(decl, ctx));
        }

        diagnostics
    }
}

impl RuleVisitor for AvoidDynamic {
    // Called by visit_declaration for each type annotation
    fn visit_type_annotation(&self, node: &TypeAnnotation, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diagnostics = vec![];

        if node.is_dynamic() {
            diagnostics.push(Diagnostic {
                rule: self.name(),
                severity: Severity::Warning,
                message: "Avoid using 'dynamic'. Use a specific type instead.".to_string(),
                file_path: ctx.file_path.to_string_lossy().to_string(),
                span: node.span(),
            });
        }

        diagnostics
    }
}

impl AvoidDynamic {
    /// Recursively visit a Declaration and collect diagnostics.
    fn visit_declaration(&self, decl: &Declaration, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = vec![];

        match decl {
            Declaration::Class(class) => {
                // Visit all class members
                for member in &class.members {
                    diags.extend(self.visit_class_member(member, ctx));
                }
            }
            Declaration::Function(func) => {
                // Check return type
                diags.extend(self.visit_type_annotation(&func.return_type, ctx));

                // Check parameter types
                for param in &func.parameters {
                    diags.extend(self.visit_type_annotation(&param.type_annotation, ctx));
                }

                // Visit function body for local variable declarations
                if let Some(body) = &func.body {
                    diags.extend(self.visit_block_statement(body, ctx));
                }
            }
            Declaration::Mixin(mixin) => {
                for member in &mixin.members {
                    diags.extend(self.visit_class_member(member, ctx));
                }
            }
            Declaration::Extension(ext) => {
                for member in &ext.members {
                    diags.extend(self.visit_class_member(member, ctx));
                }
            }
            _ => {
                // Other declaration types (typedef, enum, etc.)
            }
        }

        diags
    }

    /// Visit a class member (field, method, constructor, getter, setter).
    fn visit_class_member(&self, member: &ClassMember, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = vec![];

        match member {
            ClassMember::Field(field) => {
                // Check field type annotation
                diags.extend(self.visit_type_annotation(&field.type_annotation, ctx));
            }
            ClassMember::Method(method) => {
                // Check return type
                diags.extend(self.visit_type_annotation(&method.return_type, ctx));

                // Check parameter types
                for param in &method.parameters {
                    diags.extend(self.visit_type_annotation(&param.type_annotation, ctx));
                }

                // Visit method body
                if let Some(body) = &method.body {
                    diags.extend(self.visit_block_statement(body, ctx));
                }
            }
            ClassMember::Constructor(ctor) => {
                // Constructors don't have return types, but parameters have types
                for param in &ctor.parameters {
                    diags.extend(self.visit_type_annotation(&param.type_annotation, ctx));
                }

                if let Some(body) = &ctor.body {
                    diags.extend(self.visit_block_statement(body, ctx));
                }
            }
            _ => {}
        }

        diags
    }

    /// Visit a block statement, checking for local variable declarations.
    fn visit_block_statement(&self, block: &BlockStatement, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = vec![];

        for stmt in &block.statements {
            // Check for variable declarations
            if let Statement::VariableDeclaration(var_decl) = stmt {
                diags.extend(self.visit_type_annotation(&var_decl.type_annotation, ctx));
            }

            // Recursively visit nested blocks (if, for, while, etc.)
            // TODO: expand for nested block statements
        }

        diags
    }
}

// Stub AST types (defined in M1 in jdlint_syntax)
// These are placeholders; actual definitions come during parser M1.

/// TODO: defined in jdlint_syntax::ast (M1)
pub struct Declaration {
    // TODO
}

/// TODO: defined in jdlint_syntax::ast (M1)
pub enum Declaration {
    Class(ClassDeclaration),
    Function(FunctionDeclaration),
    Mixin(MixinDeclaration),
    Extension(ExtensionDeclaration),
    // ... others
}

/// TODO: defined in jdlint_syntax::ast (M1)
pub struct ClassDeclaration {
    pub members: Vec<ClassMember>,
}

/// TODO: defined in jdlint_syntax::ast (M1)
pub enum ClassMember {
    Field(FieldDeclaration),
    Method(MethodDeclaration),
    Constructor(ConstructorDeclaration),
}

/// TODO: defined in jdlint_syntax::ast (M1)
pub struct FieldDeclaration {
    pub type_annotation: TypeAnnotation,
}

/// TODO: defined in jdlint_syntax::ast (M1)
pub struct MethodDeclaration {
    pub return_type: TypeAnnotation,
    pub parameters: Vec<FormalParameter>,
    pub body: Option<BlockStatement>,
}

/// TODO: defined in jdlint_syntax::ast (M1)
pub struct ConstructorDeclaration {
    pub parameters: Vec<FormalParameter>,
    pub body: Option<BlockStatement>,
}

/// TODO: defined in jdlint_syntax::ast (M1)
pub struct FunctionDeclaration {
    pub return_type: TypeAnnotation,
    pub parameters: Vec<FormalParameter>,
    pub body: Option<BlockStatement>,
}

/// TODO: defined in jdlint_syntax::ast (M1)
pub struct FormalParameter {
    pub type_annotation: TypeAnnotation,
}

/// TODO: defined in jdlint_syntax::ast (M1)
pub struct TypeAnnotation {
    // ... fields ...
}

impl TypeAnnotation {
    /// Returns true if this type annotation is `dynamic`.
    fn is_dynamic(&self) -> bool {
        // TODO: M1 implementation
        false
    }

    /// Returns the byte span of this type annotation in source.
    fn span(&self) -> Span {
        // TODO: M1 implementation
        Span { start: 0, end: 0 }
    }
}

/// TODO: defined in jdlint_syntax::ast (M1)
pub enum Statement {
    VariableDeclaration(VariableDeclaration),
    // ... others
}

/// TODO: defined in jdlint_syntax::ast (M1)
pub struct VariableDeclaration {
    pub type_annotation: TypeAnnotation,
}

/// TODO: defined in jdlint_syntax::ast (M1)
pub struct BlockStatement {
    pub statements: Vec<Statement>,
}
```

### 4.2 Registration in RuleRegistry

Rules are registered during application startup:

```rust
// File: crates/jdlint/src/main.rs or crates/jdlint_cli/src/run.rs

use jdlint_analyze::{Rule, RuleRegistry};
use jdlint_rules::avoid_dynamic::AvoidDynamic;

fn main() {
    let mut registry = RuleRegistry::new();

    // Register rules
    registry.register(Box::new(AvoidDynamic));
    // registry.register(Box::new(NoEmptyBlock));
    // ... more rules

    // Use registry to analyze files
    analyze_files(registry);
}
```

---

## 5. Thread Safety Guarantees

### 5.1 What is Safe ✅

| Operation | Safe? | Why |
|-----------|-------|-----|
| Reading `ctx.file_path`, `ctx.source`, `ctx.config` | ✅ Yes | Immutable borrows; Rayon ensures no data races |
| Accumulating diagnostics in local `Vec<Diagnostic>` | ✅ Yes | `Vec` is owned by the rule's `analyze()` call; no sharing |
| Calling other immutable methods on `self` | ✅ Yes | No mutable state accessed |
| Cloning immutable config structs | ✅ Yes | Cloned data is local to `analyze()` |

### 5.2 What is NOT Safe ❌

| Operation | Safe? | Why |
|-----------|-------|-----|
| Storing mutable state in rule struct (e.g., `Arc<Mutex<Vec<Diagnostic>>>`) | ❌ No | Serializes all rules across all files; defeats Rayon parallelism |
| Using `&mut self` in `analyze()` or visitor methods | ❌ No | Violates trait signature; Rust compiler rejects |
| Writing to global state (statics, thread-local storage) | ❌ No | Data races and undefined behavior in parallel context |
| Cloning and modifying `ctx` | ❌ No | `ctx` is immutable; you cannot call mutable methods on it |

### 5.3 Rayon Parallelism Model

```
┌─────────────────────────────────────────────────────────────┐
│  Analyze Engine (Rayon work-stealing)                      │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  File1.dart ──→ [ Rule1,Rule2,Rule3 ] ──→ Diags1           │
│  File2.dart ──→ [ Rule1,Rule2,Rule3 ] ──→ Diags2           │
│  File3.dart ──→ [ Rule1,Rule2,Rule3 ] ──→ Diags3           │
│                                                              │
│  Rules executed in parallel across files:                   │
│  - File1's Rule1 runs on Thread A                           │
│  - File2's Rule1 runs on Thread B (simultaneously)          │
│  - Rule instances are immutable; no locking needed          │
│                                                              │
└─────────────────────────────────────────────────────────────┘
```

Each rule's `analyze()` is called **once per file**, **in parallel**. Thread safety is guaranteed because:
1. Rules are `Send + Sync` (no mutable shared state)
2. Each file gets its own `AnalyzeContext`
3. Diagnostics are accumulated locally and returned (no global state mutation)

---

## 6. Future: Per-Rule Parallelism (M0.5.4 Contingency)

If per-rule parallelism becomes necessary (multiple threads analyzing the same file), the contract evolves:

### Current (M0.5–M4): Per-File Parallelism

```rust
// Rule instance is shared; analyze() called once per file
let rules: Vec<Box<dyn Rule>> = /* ... */;

rayon::scope(|s| {
    for file in files {
        for rule in &rules {
            s.spawn(|_| {
                let diags = rule.analyze(&program, &ctx);
                // accumulate diags
            });
        }
    }
});
```

### Future (if needed): Per-Rule + Per-File Parallelism

If finer-grained parallelism is required, the contract could evolve:

```rust
/// HYPOTHETICAL (M0.5.4): Extended for per-rule parallelism.
/// NOT implemented in M2; design only for preemptive awareness.
pub struct AnalyzeContext<'a> {
    pub file_path: &'a Path,
    pub source: &'a str,
    pub config: &'a JdlintConfig,
    pub diag_sink: Option<Arc<Mutex<Vec<Diagnostic>>>>, // Shared diagnostic buffer
}

pub trait Rule: Send + Sync {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        // Option 1 (M2–M4): Return diagnostics (current)
        // Option 2 (if per-rule parallelism): Push to diag_sink if present
        vec![]
    }
}
```

**Preemptive requirement:** Rules MUST NOT use mutable self today (already enforced). This ensures compatibility with future per-rule parallelism without changes to rule code.

---

## 7. Rule Implementation Checklist

Use this checklist when implementing a new rule:

- [ ] **Name**: Rule ID is snake_case (e.g., `avoid_dynamic`, `no_empty_block`)
- [ ] **Struct**: Create a unit struct or struct with immutable fields (e.g., `pub struct MyRule { threshold: usize }`)
- [ ] **Rule trait**: Implement `Rule`, return correct `name()` and implement `analyze()`
- [ ] **RuleVisitor trait** (optional): If using visitor pattern, implement `RuleVisitor` with override methods
- [ ] **No mutable self**: All visit methods and `analyze()` take `&self`, not `&mut self`
- [ ] **Immutable config**: Configuration is set at construction, not mutated later
- [ ] **Diagnostics**: Each violation pushes a `Diagnostic` with rule name, severity, message, file_path, span
- [ ] **Span calculation**: Use AST node `.span()` or calculate byte offsets from `ctx.source`
- [ ] **Error handling**: On error, log to stderr and return empty `Vec<Diagnostic>` (conservative)
- [ ] **File filtering**: If rule skips certain files (test, generated), check `ctx.file_path` before analyzing
- [ ] **Config access**: If rule has config options, use `ctx.config` (M3+) or struct fields
- [ ] **Tests**: Write tests in `crates/jdlint_rules/tests/` with golden fixtures
- [ ] **Registration**: Add `registry.register(Box::new(MyRule::new(...)))` in main initialization
- [ ] **Documentation**: Add rule to `RULES.md` or project wiki with example violations and fixes

---

## 8. AST Node Types (M1 Definitions)

The following AST node types will be defined in M1 (`jdlint_syntax::ast`). Rules depend on these:

**Placeholder list — actual definitions in M1:**

```rust
// Top-level
pub struct Program {
    pub declarations: Vec<Declaration>,
}

pub enum Declaration {
    Class(ClassDeclaration),
    Mixin(MixinDeclaration),
    Extension(ExtensionDeclaration),
    Enum(EnumDeclaration),
    Typedef(TypedefDeclaration),
    Function(FunctionDeclaration),
    Variable(VariableDeclaration),
    Import(ImportDirective),
    Export(ExportDirective),
    Part(PartDirective),
    PartOf(PartOfDirective),
}

pub struct ClassDeclaration {
    pub name: String,
    pub type_parameters: Vec<TypeParameter>,
    pub superclass: Option<TypeAnnotation>,
    pub mixins: Vec<TypeAnnotation>,
    pub interfaces: Vec<TypeAnnotation>,
    pub members: Vec<ClassMember>,
}

pub enum ClassMember {
    Field(FieldDeclaration),
    Method(MethodDeclaration),
    Getter(GetterDeclaration),
    Setter(SetterDeclaration),
    Constructor(ConstructorDeclaration),
}

pub struct TypeAnnotation {
    // TODO: M1
}

pub struct FormalParameter {
    pub name: String,
    pub type_annotation: TypeAnnotation,
    pub is_required: bool,
    pub default_value: Option<Expression>,
}

// ... and many more (see jdlint_syntax::ast in M1 for full list)
```

---

## 9. Diagnostic Severity Levels

Rules select severity when emitting diagnostics:

```rust
pub enum Severity {
    /// Error: rule violation that must be fixed (blocks CI)
    Error,

    /// Warning: rule violation that should be fixed (visible to developer)
    Warning,

    /// Info: rule violation that might be worth considering (lowest priority)
    Info,
}
```

**Guidance:**

- **Error**: Use for correctness violations (null safety, type mismatches) or security issues
- **Warning**: Use for style, performance, maintainability issues (most rules)
- **Info**: Use for optional suggestions that don't affect program behavior

Example:

```rust
Diagnostic {
    rule: "avoid_dynamic",
    severity: Severity::Warning,  // Style recommendation
    message: "Avoid using 'dynamic'".to_string(),
    file_path: "lib/main.dart".to_string(),
    span: Span { start: 42, end: 49 },
}
```

---

## 10. Common Patterns

### 10.1 Optional Fields in Declarations

Some AST nodes have optional fields (e.g., optional type annotations, return types):

```rust
fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
    let mut diags = vec![];

    for decl in &program.declarations {
        if let Declaration::Variable(var) = decl {
            // Type annotation might be absent (inferred)
            if let Some(type_ann) = &var.type_annotation {
                diags.extend(self.check_type_annotation(type_ann, ctx));
            }
        }
    }

    diags
}
```

### 10.2 Collecting Diagnostics from Recursive Calls

Extend a mutable `Vec` to accumulate diagnostics:

```rust
let mut diags = vec![];

for decl in &program.declarations {
    diags.extend(self.visit_declaration(decl, ctx));
}

diags
```

### 10.3 Early Returns for Optimization

Skip expensive checks if a simple condition fails:

```rust
fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
    // Skip test files
    if ctx.file_path.to_string_lossy().ends_with("_test.dart") {
        return vec![];
    }

    let mut diags = vec![];
    // ... expensive analysis ...
    diags
}
```

### 10.4 Configuration-Driven Behavior

Rules read `ctx.config` or struct fields for runtime behavior:

```rust
pub struct NoMagicNumber {
    threshold: usize,
    exclude: Vec<u64>,
}

impl Rule for NoMagicNumber {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = vec![];

        // Use self.threshold (immutable; set at construction)
        // Use self.exclude (immutable; set at construction)

        for decl in &program.declarations {
            if let Declaration::Function(func) = decl {
                diags.extend(self.check_function(func, ctx));
            }
        }

        diags
    }

    fn name(&self) -> &'static str {
        "no_magic_number"
    }
}

impl NoMagicNumber {
    pub fn new(threshold: usize, exclude: Vec<u64>) -> Self {
        Self { threshold, exclude }
    }
}
```

---

## 11. Testing Rules

Each rule must have tests in `crates/jdlint_rules/tests/`.

### 11.1 Test Structure

```rust
// File: crates/jdlint_rules/tests/avoid_dynamic_tests.rs

#[cfg(test)]
mod tests {
    use jdlint_rules::avoid_dynamic::AvoidDynamic;
    use jdlint_analyze::{Rule, AnalyzeContext};
    use jdlint_syntax::Program;
    use std::path::Path;

    #[test]
    fn test_flags_dynamic_type_annotation() {
        let rule = AvoidDynamic;
        let source = r#"
            dynamic getValue() {
                return null;
            }
        "#;
        let program = parse_dart_source(source); // Helper (M1)
        let config = JdlintConfig::default();
        let ctx = AnalyzeContext {
            file_path: Path::new("test.dart"),
            source,
            config: &config,
        };

        let diags = rule.analyze(&program, &ctx);

        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].rule, "avoid_dynamic");
        assert!(diags[0].message.contains("dynamic"));
    }

    #[test]
    fn test_allows_specific_types() {
        let rule = AvoidDynamic;
        let source = r#"
            String getValue() {
                return "hello";
            }
        "#;
        let program = parse_dart_source(source);
        let config = JdlintConfig::default();
        let ctx = AnalyzeContext {
            file_path: Path::new("test.dart"),
            source,
            config: &config,
        };

        let diags = rule.analyze(&program, &ctx);

        assert_eq!(diags.len(), 0);
    }
}
```

### 11.2 Golden Test Fixtures

For complex rules, use golden fixtures:

```
crates/jdlint_rules/tests/fixtures/
├── avoid_dynamic/
│   ├── violations.dart        (code that should trigger violations)
│   └── clean.dart             (code that should not trigger violations)
├── no_empty_block/
│   ├── violations.dart
│   └── clean.dart
└── ...
```

---

## 12. FAQ

### Q: Can a rule store state between analyze() calls?

**A:** No. Rule instances are shared across all files and Rayon workers. Mutable state would require synchronization (locks) and would serialize parallel execution. Store configuration in struct fields instead.

### Q: What if my rule needs information from multiple files?

**A:** Current architecture (M2–M4) analyzes files independently. If a rule needs cross-file analysis, it must be deferred to M5+ as a separate analysis phase. For now, rules are single-file only.

### Q: How do I handle ambiguous AST nodes (e.g., is this a field or a getter)?

**A:** AST node types are explicit. Use Rust's exhaustive match to ensure you handle all variants:

```rust
match class_member {
    ClassMember::Field(f) => { /* ... */ }
    ClassMember::Method(m) => { /* ... */ }
    ClassMember::Getter(g) => { /* ... */ }
    ClassMember::Setter(s) => { /* ... */ }
    ClassMember::Constructor(c) => { /* ... */ }
}
```

### Q: Can I call another rule from my rule?

**A:** Rules are independent. If you need shared logic, extract it to a helper module:

```rust
// crates/jdlint_rules/src/helpers/type_checker.rs
pub fn is_dynamic(type_ann: &TypeAnnotation) -> bool { /* ... */ }

// crates/jdlint_rules/src/avoid_dynamic.rs
use crate::helpers::type_checker::is_dynamic;
```

### Q: What severity should I use?

**A:** See section 9. Most rules are `Warning`. Use `Error` for critical issues (security, correctness) and `Info` for light suggestions.

### Q: How do I access the config for my rule?

**A:** In M2–M3, use struct fields:

```rust
pub struct MyRule { threshold: usize }
impl MyRule { pub fn new(threshold: usize) -> Self { Self { threshold } } }
impl Rule for MyRule {
    fn analyze(&self, ...) -> Vec<Diagnostic> {
        if value > self.threshold { /* violation */ }
    }
}
```

In M3+, use `ctx.config.rules.my_rule_config` (TBD).

### Q: What if the AST is missing for a part of the code (parse error)?

**A:** The parser emits errors to stderr and returns a best-effort AST (M1 design). Rules analyze what exists. If a critical parse fails, the entire file's analysis is skipped (conservative).

---

## Summary

This document locks the trait contracts for `Rule`, `RuleVisitor`, and `AnalyzeContext` at M0.5. Rule engineers implementing the 60+ lint rules in M2 and beyond have:

1. **Clear trait signatures** with immutability guarantees
2. **Thread-safe parallelism** via Rayon (per-file, no mutable self)
3. **Per-file context** (source, path, config) for analysis
4. **Example implementation** (`avoid_dynamic`) showing patterns
5. **Checklist and FAQ** for common implementation questions

Rules are **not** to be modified after M2; new rules follow this contract exactly.

---

**Locked by:** Architecture Review (M0.5)  
**Valid from:** 2026-06-09  
**Next review:** M2 completion (end of week 2)
