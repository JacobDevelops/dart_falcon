use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use std::collections::HashMap;

pub struct AvoidUnnecessaryTypeCasts;

impl AvoidUnnecessaryTypeCasts {
    fn collect_local_vars(&self, stmts: &[Stmt]) -> HashMap<String, DartType> {
        let mut var_types = HashMap::new();

        for stmt in stmts {
            match stmt {
                Stmt::LocalVar(LocalVarDecl {
                    var_type,
                    declarators,
                    ..
                }) => {
                    if let Some(var_t) = var_type
                        && let DartType::Named(named) = var_t
                            && !named.is_nullable {
                                for declarator in declarators {
                                    var_types.insert(declarator.name.name.clone(), var_t.clone());
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

                    if let Some(else_stmt) = else_branch
                        && let Stmt::Block(Block {
                            stmts: else_stmts, ..
                        }) = else_stmt.as_ref()
                        {
                            let nested = self.collect_local_vars(else_stmts);
                            var_types.extend(nested);
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

    fn collect_class_fields(class: &ClassDecl) -> HashMap<String, DartType> {
        let mut map = HashMap::new();
        for member in &class.members {
            if let ClassMember::Field(field) = member
                && let Some(field_type) = &field.field_type
                    && let DartType::Named(named) = field_type
                        && !named.is_nullable {
                            for declarator in &field.declarators {
                                map.insert(declarator.name.name.clone(), field_type.clone());
                            }
                        }
        }
        map
    }

    fn types_match(declared: &DartType, cast_type: &DartType) -> bool {
        if let (DartType::Named(decl), DartType::Named(cast)) = (declared, cast_type) {
            if decl.is_nullable { return false; }
            let decl_name = decl.segments.first().map(|s| s.name.as_str());
            let cast_name = cast.segments.first().map(|s| s.name.as_str());
            if decl_name != cast_name { return false; }
            if cast.type_args.len() != decl.type_args.len() { return false; }
            if cast.type_args.is_empty() { return true; }
            cast.type_args.iter().zip(decl.type_args.iter()).all(|(c, d)| {
                if let (DartType::Named(cn), DartType::Named(dn)) = (c, d) {
                    cn.segments.first().map(|s| s.name.as_str()) == dn.segments.first().map(|s| s.name.as_str())
                } else {
                    false
                }
            })
        } else {
            false
        }
    }

    fn check_as_expr(&self, expr: &Expr, dart_type: &DartType, var_types: &HashMap<String, DartType>) -> bool {
        if let Expr::Ident(Identifier { name, .. }) = expr
            && let Some(declared) = var_types.get(name) {
                return Self::types_match(declared, dart_type);
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
                        CascadeOp::Field(_, _) => {}
                        CascadeOp::Assign(target, _, value) => {
                            self.visit_exprs(target, f);
                            self.visit_exprs(value, f);
                        }
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
                Stmt::Return(ReturnStmt { value: Some(v), .. }) => {
                    let mut expr_visitor = |_: &Expr| {};
                    self.visit_exprs(v, &mut expr_visitor);
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

impl Rule for AvoidUnnecessaryTypeCasts {
    fn name(&self) -> &'static str {
        "avoid-unnecessary-type-casts"
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
                                _ => vec![],
                            };
                            for expr in root_exprs {
                                self.visit_exprs(expr, &mut |e| {
                                    if let Expr::As { expr, dart_type, span } = e
                                        && self.check_as_expr(expr, dart_type, &var_types) {
                                            diagnostics.push(Diagnostic::new(
                                                "avoid-unnecessary-type-casts",
                                                Severity::Warning,
                                                "Unnecessary type cast — variable is already known to be this type",
                                                ctx.file_path.to_string_lossy().into_owned(),
                                                DiagSpan { start: span.start, end: span.end },
                                            ));
                                        }
                                });
                            }
                        });
                    }
                }
                TopLevelDecl::Class(class_decl) => {
                    let class_fields = Self::collect_class_fields(class_decl);
                    for member in &class_decl.members {
                        match member {
                            ClassMember::Method(method_decl) => {
                                if let Some(FunctionBody::Block(block)) = &method_decl.body {
                                    let mut var_types = class_fields.clone();
                                    var_types.extend(self.collect_local_vars(&block.stmts));
                                    self.visit_stmts(&block.stmts, &mut |stmt| {
                                        let root_exprs: Vec<&Expr> = match stmt {
                                            Stmt::Expr(ExprStmt { expr, .. }) => vec![expr],
                                            Stmt::LocalVar(lv) => lv.declarators.iter()
                                                .filter_map(|d| d.initializer.as_ref())
                                                .collect(),
                                            _ => vec![],
                                        };
                                        for expr in root_exprs {
                                            self.visit_exprs(expr, &mut |e| {
                                                if let Expr::As { expr, dart_type, span } = e
                                                    && self.check_as_expr(expr, dart_type, &var_types) {
                                                        diagnostics.push(Diagnostic::new(
                                                            "avoid-unnecessary-type-casts",
                                                            Severity::Warning,
                                                            "Unnecessary type cast — variable is already known to be this type",
                                                            ctx.file_path.to_string_lossy().into_owned(),
                                                            DiagSpan { start: span.start, end: span.end },
                                                        ));
                                                    }
                                            });
                                        }
                                    });
                                }
                            }
                            ClassMember::Getter(getter_decl) => {
                                if let Some(FunctionBody::Block(block)) = &getter_decl.body {
                                    let mut var_types = class_fields.clone();
                                    var_types.extend(self.collect_local_vars(&block.stmts));
                                    self.visit_stmts(&block.stmts, &mut |stmt| {
                                        let root_exprs: Vec<&Expr> = match stmt {
                                            Stmt::Expr(ExprStmt { expr, .. }) => vec![expr],
                                            Stmt::LocalVar(lv) => lv.declarators.iter()
                                                .filter_map(|d| d.initializer.as_ref())
                                                .collect(),
                                            _ => vec![],
                                        };
                                        for expr in root_exprs {
                                            self.visit_exprs(expr, &mut |e| {
                                                if let Expr::As { expr, dart_type, span } = e
                                                    && self.check_as_expr(expr, dart_type, &var_types) {
                                                        diagnostics.push(Diagnostic::new(
                                                            "avoid-unnecessary-type-casts",
                                                            Severity::Warning,
                                                            "Unnecessary type cast — variable is already known to be this type",
                                                            ctx.file_path.to_string_lossy().into_owned(),
                                                            DiagSpan { start: span.start, end: span.end },
                                                        ));
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
