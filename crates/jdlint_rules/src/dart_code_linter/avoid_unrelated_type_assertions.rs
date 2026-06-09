use jdlint_analyze::{AnalyzeContext, Rule};
use jdlint_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use jdlint_syntax::ast::*;
use std::collections::HashMap;

pub struct AvoidUnrelatedTypeAssertions;

impl AvoidUnrelatedTypeAssertions {
    fn is_incompatible_type(&self, literal_category: &str, type_name: &str) -> bool {
        match literal_category {
            "String" => matches!(type_name, "int" | "double" | "bool" | "List" | "Map" | "Set"),
            "int" => matches!(type_name, "String" | "bool" | "List" | "Map" | "Set" | "double"),
            "double" => matches!(type_name, "String" | "bool" | "List" | "Map" | "Set" | "int"),
            "bool" => matches!(type_name, "String" | "int" | "double" | "List" | "Map" | "Set"),
            "List" => matches!(type_name, "String" | "int" | "double" | "bool" | "Map"),
            "Map" => matches!(type_name, "String" | "int" | "double" | "bool" | "List"),
            "Set" => matches!(type_name, "String" | "int" | "double" | "bool" | "List" | "Map"),
            _ => false,
        }
    }

    fn get_literal_category(expr: &Expr) -> Option<&'static str> {
        match expr {
            Expr::StringLit(_) => Some("String"),
            Expr::IntLit { .. } => Some("int"),
            Expr::DoubleLit { .. } => Some("double"),
            Expr::BoolLit { .. } => Some("bool"),
            Expr::List { .. } => Some("List"),
            Expr::Map { .. } => Some("Map"),
            Expr::Set { .. } => Some("Set"),
            _ => None,
        }
    }

    fn get_first_segment(dart_type: &DartType) -> Option<String> {
        match dart_type {
            DartType::Named(named) => {
                if let Some(first) = named.segments.first() {
                    Some(first.name.clone())
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn infer_type_from_expr(expr: &Expr) -> Option<String> {
        match expr {
            Expr::StringLit(_) => Some("String".to_string()),
            Expr::IntLit { .. } => Some("int".to_string()),
            Expr::DoubleLit { .. } => Some("double".to_string()),
            Expr::BoolLit { .. } => Some("bool".to_string()),
            Expr::List { .. } => Some("List".to_string()),
            Expr::Map { .. } => Some("Map".to_string()),
            Expr::Set { .. } => Some("Set".to_string()),
            _ => None,
        }
    }

    fn collect_local_vars(&self, stmts: &[Stmt]) -> HashMap<String, String> {
        let mut var_types = HashMap::new();

        for stmt in stmts {
            match stmt {
                Stmt::LocalVar(LocalVarDecl {
                    var_type,
                    declarators,
                    ..
                }) => {
                    for declarator in declarators {
                        if let Some(init_expr) = &declarator.initializer {
                            if let Some(inferred) = Self::infer_type_from_expr(init_expr) {
                                var_types.insert(declarator.name.name.clone(), inferred);
                            }
                        }

                        if let Some(var_t) = var_type {
                            if let Some(type_name) = Self::get_first_segment(var_t) {
                                var_types.insert(declarator.name.name.clone(), type_name);
                            }
                        }
                    }
                }
                Stmt::If(IfStmt {
                    then_branch,
                    else_branch,
                    ..
                }) => {
                    if let Stmt::Block(Block { stmts: then_stmts, .. }) = then_branch.as_ref() {
                        let nested = self.collect_local_vars(then_stmts);
                        var_types.extend(nested);
                    }

                    if let Some(else_stmt) = else_branch {
                        if let Stmt::Block(Block {
                            stmts: else_stmts, ..
                        }) = else_stmt.as_ref()
                        {
                            let nested = self.collect_local_vars(else_stmts);
                            var_types.extend(nested);
                        }
                    }
                }
                Stmt::Block(Block { stmts, .. }) => {
                    let nested = self.collect_local_vars(stmts);
                    var_types.extend(nested);
                }
                _ => {}
            }
        }

        var_types
    }

    fn check_is_expr(&self, expr: &Expr, dart_type: &DartType, var_types: &HashMap<String, String>) -> bool {
        if let Some(category) = Self::get_literal_category(expr) {
            if let Some(type_name) = Self::get_first_segment(dart_type) {
                return self.is_incompatible_type(category, &type_name);
            }
        }

        if let Expr::Ident(Identifier { name, .. }) = expr {
            if let Some(var_type) = var_types.get(name) {
                if let Some(assert_type) = Self::get_first_segment(dart_type) {
                    return self.is_incompatible_type(var_type, &assert_type);
                }
            }
        }

        false
    }

    fn visit_exprs<F>(&self, expr: &Expr, f: &mut F)
    where
        F: FnMut(&Expr),
    {
        f(expr);

        match expr {
            Expr::Unary { operand, .. } => self.visit_exprs(operand, f),
            Expr::PostfixIncDec { operand, .. } => self.visit_exprs(operand, f),
            Expr::Binary { left, right, .. } => {
                self.visit_exprs(left, f);
                self.visit_exprs(right, f);
            }
            Expr::Assign { target, value, .. } => {
                self.visit_exprs(target, f);
                self.visit_exprs(value, f);
            }
            Expr::Conditional {
                condition,
                then_expr,
                else_expr,
                ..
            } => {
                self.visit_exprs(condition, f);
                self.visit_exprs(then_expr, f);
                self.visit_exprs(else_expr, f);
            }
            Expr::Is { expr, .. } => self.visit_exprs(expr, f),
            Expr::As { expr, .. } => self.visit_exprs(expr, f),
            Expr::Field { object, .. } => self.visit_exprs(object, f),
            Expr::Index { object, index, .. } => {
                self.visit_exprs(object, f);
                self.visit_exprs(index, f);
            }
            Expr::Call { callee, args, .. } => {
                self.visit_exprs(callee, f);
                for arg in &args.positional {
                    self.visit_exprs(arg, f);
                }
                for named_arg in &args.named {
                    self.visit_exprs(&named_arg.value, f);
                }
            }
            Expr::Cascade { object, sections, .. } => {
                self.visit_exprs(object, f);
                for section in sections {
                    match &section.op {
                        CascadeOp::Call(_, _, args) => {
                            for arg in &args.positional {
                                self.visit_exprs(arg, f);
                            }
                            for named_arg in &args.named {
                                self.visit_exprs(&named_arg.value, f);
                            }
                        }
                        CascadeOp::Index(index, _) => self.visit_exprs(index, f),
                        CascadeOp::Assign(target, _, value) => {
                            self.visit_exprs(target, f);
                            self.visit_exprs(value, f);
                        }
                        CascadeOp::Field(_, _) => {}
                    }
                }
            }
            Expr::List { elements, .. } => {
                for elem in elements {
                    match elem {
                        CollectionElement::Expr(e) => self.visit_exprs(e, f),
                        CollectionElement::Spread { expr, .. } => self.visit_exprs(expr, f),
                        CollectionElement::If {
                            condition,
                            then_elem,
                            else_elem,
                            ..
                        } => {
                            if let IfCondition::Expr(cond) = condition {
                                self.visit_exprs(cond, f);
                            }
                            match then_elem.as_ref() {
                                CollectionElement::Expr(e) => self.visit_exprs(e, f),
                                CollectionElement::Spread { expr, .. } => self.visit_exprs(expr, f),
                                _ => {}
                            }
                            if let Some(else_e) = else_elem {
                                match else_e.as_ref() {
                                    CollectionElement::Expr(e) => self.visit_exprs(e, f),
                                    CollectionElement::Spread { expr, .. } => {
                                        self.visit_exprs(expr, f)
                                    }
                                    _ => {}
                                }
                            }
                        }
                        CollectionElement::For { element, .. } => match element.as_ref() {
                            CollectionElement::Expr(e) => self.visit_exprs(e, f),
                            CollectionElement::Spread { expr, .. } => self.visit_exprs(expr, f),
                            _ => {}
                        },
                    }
                }
            }
            Expr::Map { entries, .. } => {
                for entry in entries {
                    self.visit_exprs(&entry.key, f);
                    self.visit_exprs(&entry.value, f);
                }
            }
            Expr::Set { elements, .. } => {
                for elem in elements {
                    match elem {
                        CollectionElement::Expr(e) => self.visit_exprs(e, f),
                        CollectionElement::Spread { expr, .. } => self.visit_exprs(expr, f),
                        CollectionElement::If {
                            condition,
                            then_elem,
                            else_elem,
                            ..
                        } => {
                            if let IfCondition::Expr(cond) = condition {
                                self.visit_exprs(cond, f);
                            }
                            match then_elem.as_ref() {
                                CollectionElement::Expr(e) => self.visit_exprs(e, f),
                                CollectionElement::Spread { expr, .. } => self.visit_exprs(expr, f),
                                _ => {}
                            }
                            if let Some(else_e) = else_elem {
                                match else_e.as_ref() {
                                    CollectionElement::Expr(e) => self.visit_exprs(e, f),
                                    CollectionElement::Spread { expr, .. } => {
                                        self.visit_exprs(expr, f)
                                    }
                                    _ => {}
                                }
                            }
                        }
                        CollectionElement::For { element, .. } => match element.as_ref() {
                            CollectionElement::Expr(e) => self.visit_exprs(e, f),
                            CollectionElement::Spread { expr, .. } => self.visit_exprs(expr, f),
                            _ => {}
                        },
                    }
                }
            }
            Expr::Record { fields, .. } => {
                for field in fields {
                    self.visit_exprs(&field.value, f);
                }
            }
            Expr::Await { expr, .. } => self.visit_exprs(expr, f),
            Expr::Throw { expr, .. } => self.visit_exprs(expr, f),
            Expr::Switch { subject, arms, .. } => {
                self.visit_exprs(subject, f);
                for arm in arms {
                    self.visit_exprs(&arm.body, f);
                }
            }
            Expr::NullAssert { operand, .. } => self.visit_exprs(operand, f),
            Expr::New { args, .. } => {
                for arg in &args.positional {
                    self.visit_exprs(arg, f);
                }
                for named_arg in &args.named {
                    self.visit_exprs(&named_arg.value, f);
                }
            }
            _ => {}
        }
    }

    fn visit_stmts(&self, stmts: &[Stmt], f: &mut impl FnMut(&Stmt)) {
        for stmt in stmts {
            f(stmt);

            match stmt {
                Stmt::Block(Block { stmts, .. }) => self.visit_stmts(stmts, f),
                Stmt::If(IfStmt {
                    condition,
                    then_branch,
                    else_branch,
                    ..
                }) => {
                    if let IfCondition::Expr(cond) = condition {
                        let mut expr_visitor = |_: &Expr| {};
                        self.visit_exprs(cond, &mut expr_visitor);
                    }
                    self.visit_stmts(&[then_branch.as_ref().clone()], f);
                    if let Some(else_stmt) = else_branch {
                        self.visit_stmts(&[else_stmt.as_ref().clone()], f);
                    }
                }
                Stmt::For(ForStmt {
                    condition,
                    update,
                    body,
                    ..
                }) => {
                    if let Some(cond) = condition {
                        let mut expr_visitor = |_: &Expr| {};
                        self.visit_exprs(cond, &mut expr_visitor);
                    }
                    for upd in update {
                        let mut expr_visitor = |_: &Expr| {};
                        self.visit_exprs(upd, &mut expr_visitor);
                    }
                    self.visit_stmts(&[body.as_ref().clone()], f);
                }
                Stmt::While(WhileStmt { condition, body, .. }) => {
                    let mut expr_visitor = |_: &Expr| {};
                    self.visit_exprs(condition, &mut expr_visitor);
                    self.visit_stmts(&[body.as_ref().clone()], f);
                }
                Stmt::DoWhile(DoWhileStmt { body, condition, .. }) => {
                    self.visit_stmts(&[body.as_ref().clone()], f);
                    let mut expr_visitor = |_: &Expr| {};
                    self.visit_exprs(condition, &mut expr_visitor);
                }
                Stmt::Switch(SwitchStmt { subject, cases, .. }) => {
                    let mut expr_visitor = |_: &Expr| {};
                    self.visit_exprs(subject, &mut expr_visitor);
                    for case in cases {
                        self.visit_stmts(&case.body, f);
                    }
                }
                Stmt::TryCatch(TryCatchStmt {
                    body,
                    catches,
                    finally,
                    ..
                }) => {
                    self.visit_stmts(&body.stmts, f);
                    for catch_clause in catches {
                        self.visit_stmts(&catch_clause.body.stmts, f);
                    }
                    if let Some(finally_block) = finally {
                        self.visit_stmts(&finally_block.stmts, f);
                    }
                }
                Stmt::Return(ReturnStmt { value, .. }) => {
                    if let Some(v) = value {
                        let mut expr_visitor = |_: &Expr| {};
                        self.visit_exprs(v, &mut expr_visitor);
                    }
                }
                Stmt::Throw(ThrowStmt { value, .. }) => {
                    let mut expr_visitor = |_: &Expr| {};
                    self.visit_exprs(value, &mut expr_visitor);
                }
                Stmt::Expr(ExprStmt { expr, .. }) => {
                    let mut expr_visitor = |_: &Expr| {};
                    self.visit_exprs(expr, &mut expr_visitor);
                }
                _ => {}
            }
        }
    }
}

impl Rule for AvoidUnrelatedTypeAssertions {
    fn name(&self) -> &'static str {
        "avoid-unrelated-type-assertions"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        for decl in &program.declarations {
            match decl {
                TopLevelDecl::Function(func_decl) => {
                    if let Some(FunctionBody::Block(block)) = &func_decl.body {
                        let var_types = self.collect_local_vars(&block.stmts);
                        self.visit_stmts(&block.stmts, &mut |stmt| {
                            let root_exprs: Vec<&Expr> = match stmt {
                                Stmt::Expr(ExprStmt { expr, .. }) => vec![expr],
                                Stmt::LocalVar(lv) => lv.declarators.iter()
                                    .filter_map(|d| d.initializer.as_ref())
                                    .collect(),
                                Stmt::If(if_stmt) => match &if_stmt.condition {
                                    IfCondition::Expr(e) => vec![e],
                                    _ => vec![],
                                },
                                Stmt::Return(ret) => match &ret.value {
                                    Some(v) => vec![v],
                                    None => vec![],
                                },
                                _ => vec![],
                            };
                            for expr in root_exprs {
                                self.visit_exprs(expr, &mut |e| {
                                    if let Expr::Is {
                                        expr,
                                        dart_type,
                                        negated: false,
                                        span,
                                    } = e
                                    {
                                        if self.check_is_expr(expr, dart_type, &var_types) {
                                            diagnostics.push(Diagnostic::new(
                                                "avoid-unrelated-type-assertions",
                                                Severity::Warning,
                                                "Type assertion can never be true — types are unrelated",
                                                ctx.file_path.to_string_lossy().into_owned(),
                                                DiagSpan {
                                                    start: span.start,
                                                    end: span.end,
                                                },
                                            ));
                                        }
                                    }
                                });
                            }
                        });
                    }
                }
                TopLevelDecl::Class(class_decl) => {
                    for member in &class_decl.members {
                        match member {
                            ClassMember::Method(method_decl) => {
                                if let Some(FunctionBody::Block(block)) = &method_decl.body {
                                    let var_types = self.collect_local_vars(&block.stmts);
                                    self.visit_stmts(&block.stmts, &mut |stmt| {
                                        let root_exprs: Vec<&Expr> = match stmt {
                                            Stmt::Expr(ExprStmt { expr, .. }) => vec![expr],
                                            Stmt::LocalVar(lv) => lv.declarators.iter()
                                                .filter_map(|d| d.initializer.as_ref())
                                                .collect(),
                                            Stmt::If(if_stmt) => match &if_stmt.condition {
                                                IfCondition::Expr(e) => vec![e],
                                                _ => vec![],
                                            },
                                            Stmt::Return(ret) => match &ret.value {
                                                Some(v) => vec![v],
                                                None => vec![],
                                            },
                                            _ => vec![],
                                        };
                                        for expr in root_exprs {
                                            self.visit_exprs(expr, &mut |e| {
                                                if let Expr::Is {
                                                    expr,
                                                    dart_type,
                                                    negated: false,
                                                    span,
                                                } = e
                                                {
                                                    if self.check_is_expr(expr, dart_type, &var_types) {
                                                        diagnostics.push(Diagnostic::new(
                                                            "avoid-unrelated-type-assertions",
                                                            Severity::Warning,
                                                            "Type assertion can never be true — types are unrelated",
                                                            ctx.file_path.to_string_lossy().into_owned(),
                                                            DiagSpan {
                                                                start: span.start,
                                                                end: span.end,
                                                            },
                                                        ));
                                                    }
                                                }
                                            });
                                        }
                                    });
                                }
                            }
                            ClassMember::Getter(getter_decl) => {
                                if let Some(FunctionBody::Block(block)) = &getter_decl.body {
                                    let var_types = self.collect_local_vars(&block.stmts);
                                    self.visit_stmts(&block.stmts, &mut |stmt| {
                                        let root_exprs: Vec<&Expr> = match stmt {
                                            Stmt::Expr(ExprStmt { expr, .. }) => vec![expr],
                                            Stmt::LocalVar(lv) => lv.declarators.iter()
                                                .filter_map(|d| d.initializer.as_ref())
                                                .collect(),
                                            Stmt::If(if_stmt) => match &if_stmt.condition {
                                                IfCondition::Expr(e) => vec![e],
                                                _ => vec![],
                                            },
                                            Stmt::Return(ret) => match &ret.value {
                                                Some(v) => vec![v],
                                                None => vec![],
                                            },
                                            _ => vec![],
                                        };
                                        for expr in root_exprs {
                                            self.visit_exprs(expr, &mut |e| {
                                                if let Expr::Is {
                                                    expr,
                                                    dart_type,
                                                    negated: false,
                                                    span,
                                                } = e
                                                {
                                                    if self.check_is_expr(expr, dart_type, &var_types) {
                                                        diagnostics.push(Diagnostic::new(
                                                            "avoid-unrelated-type-assertions",
                                                            Severity::Warning,
                                                            "Type assertion can never be true — types are unrelated",
                                                            ctx.file_path.to_string_lossy().into_owned(),
                                                            DiagSpan {
                                                                start: span.start,
                                                                end: span.end,
                                                            },
                                                        ));
                                                    }
                                                }
                                            });
                                        }
                                    });
                                }
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }

        diagnostics
    }
}
