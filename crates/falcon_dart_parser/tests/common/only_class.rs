use falcon_syntax::cst::{ClassDecl, Program, TopLevelDecl};

pub fn only_class(prog: Program<'_>) -> ClassDecl<'_> {
    match prog.declarations().first().unwrap() {
        TopLevelDecl::Class(c) => c,
        other => panic!("expected class, got {other:?}"),
    }
}
