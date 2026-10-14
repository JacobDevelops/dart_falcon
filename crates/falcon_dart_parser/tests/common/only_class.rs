use falcon_syntax::ast::{ClassDecl, Program, TopLevelDecl};

pub fn only_class(prog: &Program) -> &ClassDecl {
    match &prog.declarations[0] {
        TopLevelDecl::Class(c) => c,
        other => panic!("expected class, got {other:?}"),
    }
}
