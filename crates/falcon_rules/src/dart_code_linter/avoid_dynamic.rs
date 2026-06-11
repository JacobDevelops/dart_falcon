use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;

pub struct AvoidDynamic;

impl Rule for AvoidDynamic {
    fn name(&self) -> &'static str {
        "avoid-dynamic"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        for decl in &program.declarations {
            visit_top_level_decl(decl, &mut diagnostics, ctx);
        }

        diagnostics
    }
}

fn visit_top_level_decl(
    decl: &TopLevelDecl,
    diagnostics: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    match decl {
        TopLevelDecl::Class(class) => {
            visit_class_decl(class, diagnostics, ctx);
        }
        TopLevelDecl::Function(func) => {
            visit_function_decl(func, diagnostics, ctx);
        }
        TopLevelDecl::Variable(var) => {
            visit_top_level_var_decl(var, diagnostics, ctx);
        }
        TopLevelDecl::Mixin(mixin) => {
            for member in &mixin.members {
                visit_class_member(member, diagnostics, ctx);
            }
        }
        TopLevelDecl::MixinClass(mixin_class) => {
            for member in &mixin_class.members {
                visit_class_member(member, diagnostics, ctx);
            }
        }
        TopLevelDecl::Enum(enum_decl) => {
            for member in &enum_decl.members {
                visit_class_member(member, diagnostics, ctx);
            }
        }
        TopLevelDecl::Extension(ext) => {
            for member in &ext.members {
                visit_class_member(member, diagnostics, ctx);
            }
        }
        TopLevelDecl::ExtensionType(_) => {}
        TopLevelDecl::TypeAlias(_) => {}
        TopLevelDecl::Error(_) => {}
    }
}

fn visit_class_decl(class: &ClassDecl, diagnostics: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    for member in &class.members {
        visit_class_member(member, diagnostics, ctx);
    }
}

fn visit_class_member(
    member: &ClassMember,
    diagnostics: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    match member {
        ClassMember::Field(field) => {
            visit_field_decl(field, diagnostics, ctx);
        }
        ClassMember::Constructor(constructor) => {
            visit_formal_param_list(&constructor.params, diagnostics, ctx);
        }
        ClassMember::Method(method) => {
            visit_method_decl(method, diagnostics, ctx);
        }
        ClassMember::Getter(getter) => {
            visit_getter_decl(getter, diagnostics, ctx);
        }
        ClassMember::Setter(setter) => {
            visit_setter_decl(setter, diagnostics, ctx);
        }
        ClassMember::Operator(_) => {}
        ClassMember::Error(_) => {}
    }
}

fn visit_field_decl(field: &FieldDecl, diagnostics: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    if let Some(field_type) = &field.field_type {
        check_dart_type(field_type, diagnostics, ctx);
    }
}

fn visit_method_decl(method: &MethodDecl, diagnostics: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    if let Some(return_type) = &method.return_type {
        check_dart_type(return_type, diagnostics, ctx);
    }
    visit_formal_param_list(&method.params, diagnostics, ctx);
    if let Some(body) = &method.body {
        visit_function_body(body, diagnostics, ctx);
    }
}

fn visit_getter_decl(getter: &GetterDecl, diagnostics: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    if let Some(return_type) = &getter.return_type {
        check_dart_type(return_type, diagnostics, ctx);
    }
    if let Some(body) = &getter.body {
        visit_function_body(body, diagnostics, ctx);
    }
}

fn visit_setter_decl(setter: &SetterDecl, diagnostics: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    if let Some(param_type) = &setter.param_type {
        check_dart_type(param_type, diagnostics, ctx);
    }
}

fn visit_function_decl(
    func: &FunctionDecl,
    diagnostics: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    if let Some(return_type) = &func.return_type {
        check_dart_type(return_type, diagnostics, ctx);
    }
    visit_formal_param_list(&func.params, diagnostics, ctx);
    if let Some(body) = &func.body {
        visit_function_body(body, diagnostics, ctx);
    }
}

fn visit_top_level_var_decl(
    var: &TopLevelVarDecl,
    diagnostics: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    if let Some(var_type) = &var.var_type {
        check_dart_type(var_type, diagnostics, ctx);
    }
}

fn visit_formal_param_list(
    params: &FormalParamList,
    diagnostics: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    for param in &params.positional {
        visit_formal_param(param, diagnostics, ctx);
    }
    for param in &params.optional_positional {
        visit_formal_param(param, diagnostics, ctx);
    }
    for param in &params.named {
        visit_formal_param(param, diagnostics, ctx);
    }
}

fn visit_formal_param(
    param: &FormalParam,
    diagnostics: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    if let Some(param_type) = &param.param_type {
        check_dart_type(param_type, diagnostics, ctx);
    }
    if let Some(params) = &param.function_params {
        visit_formal_param_list(params, diagnostics, ctx);
    }
}

fn visit_function_body(
    body: &FunctionBody,
    diagnostics: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    match body {
        FunctionBody::Block(block) => {
            visit_block(block, diagnostics, ctx);
        }
        FunctionBody::Arrow(expr, _) => {
            visit_expr(expr, diagnostics, ctx);
        }
        FunctionBody::Native(..) => {}
    }
}

fn visit_block(block: &Block, diagnostics: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    for stmt in &block.stmts {
        visit_stmt(stmt, diagnostics, ctx);
    }
}

fn visit_stmt(stmt: &Stmt, diagnostics: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match stmt {
        Stmt::Block(block) => {
            visit_block(block, diagnostics, ctx);
        }
        Stmt::If(if_stmt) => {
            match &if_stmt.condition {
                IfCondition::Expr(expr) => {
                    visit_expr(expr, diagnostics, ctx);
                }
                IfCondition::Case(expr, _pattern) => {
                    visit_expr(expr, diagnostics, ctx);
                }
            }
            visit_stmt(&if_stmt.then_branch, diagnostics, ctx);
            if let Some(else_branch) = &if_stmt.else_branch {
                visit_stmt(else_branch, diagnostics, ctx);
            }
        }
        Stmt::For(for_stmt) => {
            if let Some(init) = &for_stmt.init {
                match init {
                    ForInit::VarDecl(var_decl) => {
                        visit_local_var_decl(var_decl, diagnostics, ctx);
                    }
                    ForInit::ForIn {
                        var_type, iterable, ..
                    } => {
                        if let Some(vtype) = var_type {
                            check_dart_type(vtype, diagnostics, ctx);
                        }
                        visit_expr(iterable, diagnostics, ctx);
                    }
                    ForInit::Exprs(exprs) => {
                        for expr in exprs {
                            visit_expr(expr, diagnostics, ctx);
                        }
                    }
                }
            }
            if let Some(condition) = &for_stmt.condition {
                visit_expr(condition, diagnostics, ctx);
            }
            for update in &for_stmt.update {
                visit_expr(update, diagnostics, ctx);
            }
            visit_stmt(&for_stmt.body, diagnostics, ctx);
        }
        Stmt::While(while_stmt) => {
            visit_expr(&while_stmt.condition, diagnostics, ctx);
            visit_stmt(&while_stmt.body, diagnostics, ctx);
        }
        Stmt::DoWhile(do_while) => {
            visit_stmt(&do_while.body, diagnostics, ctx);
            visit_expr(&do_while.condition, diagnostics, ctx);
        }
        Stmt::Switch(switch) => {
            visit_expr(&switch.subject, diagnostics, ctx);
        }
        Stmt::TryCatch(try_catch) => {
            visit_block(&try_catch.body, diagnostics, ctx);
            for catch_clause in &try_catch.catches {
                if let Some(exception_type) = &catch_clause.exception_type {
                    check_dart_type(exception_type, diagnostics, ctx);
                }
                visit_block(&catch_clause.body, diagnostics, ctx);
            }
            if let Some(finally_block) = &try_catch.finally {
                visit_block(finally_block, diagnostics, ctx);
            }
        }
        Stmt::Return(return_stmt) => {
            if let Some(value) = &return_stmt.value {
                visit_expr(value, diagnostics, ctx);
            }
        }
        Stmt::Throw(throw_stmt) => {
            visit_expr(&throw_stmt.value, diagnostics, ctx);
        }
        Stmt::Break(_) => {}
        Stmt::Continue(_) => {}
        Stmt::LocalVar(local_var) => {
            visit_local_var_decl(local_var, diagnostics, ctx);
        }
        Stmt::LocalFunc(local_func) => {
            visit_local_func_decl(local_func, diagnostics, ctx);
        }
        Stmt::Assert(_) => {}
        Stmt::Yield(yield_stmt) => {
            visit_expr(&yield_stmt.value, diagnostics, ctx);
        }
        Stmt::Expr(expr_stmt) => {
            visit_expr(&expr_stmt.expr, diagnostics, ctx);
        }
        Stmt::Error(_) => {}
    }
}

fn visit_local_var_decl(
    local_var: &LocalVarDecl,
    diagnostics: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    if let Some(var_type) = &local_var.var_type {
        check_dart_type(var_type, diagnostics, ctx);
    }
}

fn visit_local_func_decl(
    local_func: &LocalFuncDecl,
    diagnostics: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    if let Some(return_type) = &local_func.return_type {
        check_dart_type(return_type, diagnostics, ctx);
    }
    visit_formal_param_list(&local_func.params, diagnostics, ctx);
    visit_function_body(&local_func.body, diagnostics, ctx);
}

fn visit_expr(expr: &Expr, diagnostics: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match expr {
        Expr::IntLit { .. } => {}
        Expr::DoubleLit { .. } => {}
        Expr::StringLit(_) => {}
        Expr::BoolLit { .. } => {}
        Expr::NullLit { .. } => {}
        Expr::Ident(_) => {}
        Expr::This { .. } => {}
        Expr::Super { .. } => {}
        Expr::Unary { operand, .. } => {
            visit_expr(operand, diagnostics, ctx);
        }
        Expr::PostfixIncDec { operand, .. } => {
            visit_expr(operand, diagnostics, ctx);
        }
        Expr::Binary { left, right, .. } => {
            visit_expr(left, diagnostics, ctx);
            visit_expr(right, diagnostics, ctx);
        }
        Expr::Assign { target, value, .. } => {
            visit_expr(target, diagnostics, ctx);
            visit_expr(value, diagnostics, ctx);
        }
        Expr::Conditional {
            condition,
            then_expr,
            else_expr,
            ..
        } => {
            visit_expr(condition, diagnostics, ctx);
            visit_expr(then_expr, diagnostics, ctx);
            visit_expr(else_expr, diagnostics, ctx);
        }
        Expr::Is {
            expr, dart_type, ..
        } => {
            visit_expr(expr, diagnostics, ctx);
            check_dart_type(dart_type, diagnostics, ctx);
        }
        Expr::As {
            expr, dart_type, ..
        } => {
            visit_expr(expr, diagnostics, ctx);
            check_dart_type(dart_type, diagnostics, ctx);
        }
        Expr::Field { object, .. } => {
            visit_expr(object, diagnostics, ctx);
        }
        Expr::Index { object, index, .. } => {
            visit_expr(object, diagnostics, ctx);
            visit_expr(index, diagnostics, ctx);
        }
        Expr::Call {
            callee,
            type_args,
            args,
            ..
        } => {
            visit_expr(callee, diagnostics, ctx);
            for type_arg in type_args {
                check_dart_type(type_arg, diagnostics, ctx);
            }
            for expr in &args.positional {
                visit_expr(expr, diagnostics, ctx);
            }
            for named_arg in &args.named {
                visit_expr(&named_arg.value, diagnostics, ctx);
            }
        }
        Expr::Cascade {
            object, sections, ..
        } => {
            visit_expr(object, diagnostics, ctx);
            for section in sections {
                match &section.op {
                    CascadeOp::Call(_, type_args, args) => {
                        for type_arg in type_args {
                            check_dart_type(type_arg, diagnostics, ctx);
                        }
                        for expr in &args.positional {
                            visit_expr(expr, diagnostics, ctx);
                        }
                        for named_arg in &args.named {
                            visit_expr(&named_arg.value, diagnostics, ctx);
                        }
                    }
                    CascadeOp::Index(index, _) => {
                        visit_expr(index, diagnostics, ctx);
                    }
                    CascadeOp::Field(_, _) => {}
                    CascadeOp::Assign(target, _, value) => {
                        visit_expr(target, diagnostics, ctx);
                        visit_expr(value, diagnostics, ctx);
                    }
                }
            }
        }
        Expr::List {
            type_arg, elements, ..
        } => {
            if let Some(type_arg) = type_arg {
                check_dart_type(type_arg, diagnostics, ctx);
            }
            for elem in elements {
                visit_collection_element(elem, diagnostics, ctx);
            }
        }
        Expr::Map {
            type_args, entries, ..
        } => {
            for type_arg in type_args {
                check_dart_type(type_arg, diagnostics, ctx);
            }
            for entry in entries {
                visit_expr(&entry.key, diagnostics, ctx);
                visit_expr(&entry.value, diagnostics, ctx);
            }
        }
        Expr::Set {
            type_arg, elements, ..
        } => {
            if let Some(type_arg) = type_arg {
                check_dart_type(type_arg, diagnostics, ctx);
            }
            for elem in elements {
                visit_collection_element(elem, diagnostics, ctx);
            }
        }
        Expr::Record { fields, .. } => {
            for field in fields {
                visit_expr(&field.value, diagnostics, ctx);
            }
        }
        Expr::FuncExpr { params, body, .. } => {
            visit_formal_param_list(params, diagnostics, ctx);
            visit_function_body(body, diagnostics, ctx);
        }
        Expr::New {
            dart_type, args, ..
        } => {
            check_dart_type(dart_type, diagnostics, ctx);
            for expr in &args.positional {
                visit_expr(expr, diagnostics, ctx);
            }
            for named_arg in &args.named {
                visit_expr(&named_arg.value, diagnostics, ctx);
            }
        }
        Expr::Await { expr, .. } => {
            visit_expr(expr, diagnostics, ctx);
        }
        Expr::Throw { expr, .. } => {
            visit_expr(expr, diagnostics, ctx);
        }
        Expr::Switch { subject, .. } => {
            visit_expr(subject, diagnostics, ctx);
        }
        Expr::NullAssert { operand, .. } => {
            visit_expr(operand, diagnostics, ctx);
        }
        Expr::Error { .. } => {}
    }
}

fn visit_collection_element(
    elem: &CollectionElement,
    diagnostics: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
) {
    match elem {
        CollectionElement::Expr(expr) => {
            visit_expr(expr, diagnostics, ctx);
        }
        CollectionElement::Spread { expr, .. } => {
            visit_expr(expr, diagnostics, ctx);
        }
        CollectionElement::If {
            condition,
            then_elem,
            else_elem,
            ..
        } => {
            match condition {
                IfCondition::Expr(expr) => {
                    visit_expr(expr, diagnostics, ctx);
                }
                IfCondition::Case(expr, _) => {
                    visit_expr(expr, diagnostics, ctx);
                }
            }
            visit_collection_element(then_elem, diagnostics, ctx);
            if let Some(else_e) = else_elem {
                visit_collection_element(else_e, diagnostics, ctx);
            }
        }
        CollectionElement::For {
            var_type,
            iterable,
            element,
            ..
        } => {
            if let Some(vtype) = var_type {
                check_dart_type(vtype, diagnostics, ctx);
            }
            visit_expr(iterable, diagnostics, ctx);
            visit_collection_element(element, diagnostics, ctx);
        }
    }
}

fn check_dart_type(dart_type: &DartType, diagnostics: &mut Vec<Diagnostic>, ctx: &AnalyzeContext) {
    match dart_type {
        DartType::Dynamic { span } => {
            diagnostics.push(Diagnostic::new(
                "avoid-dynamic",
                Severity::Warning,
                "Avoid using the 'dynamic' type",
                ctx.file_path.to_string_lossy().into_owned(),
                DiagSpan {
                    start: span.start,
                    end: span.end,
                },
            ));
        }
        DartType::Named(named_type) => {
            for type_arg in &named_type.type_args {
                check_dart_type(type_arg, diagnostics, ctx);
            }
        }
        DartType::Function(func_type) => {
            if let Some(return_type) = &func_type.return_type {
                check_dart_type(return_type, diagnostics, ctx);
            }
            for param in &func_type.params {
                check_dart_type(&param.param_type, diagnostics, ctx);
            }
        }
        DartType::Record(_) => {}
        DartType::Void { .. } => {}
        DartType::Never { .. } => {}
    }
}
