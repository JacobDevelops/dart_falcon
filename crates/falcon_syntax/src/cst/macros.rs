use std::fmt;

macro_rules! typed_node {
    ($name:ident, $kind:ident) => {
        #[derive(Clone, Copy)]
        pub struct $name<'a>(pub(crate) $crate::cst::SyntaxNode<'a>);

        impl<'a> $crate::cst::AstNode<'a> for $name<'a> {
            fn cast(syntax: $crate::cst::SyntaxNode<'a>) -> Option<Self> {
                (syntax.kind() == $crate::cst::NodeKind::$kind).then_some(Self(syntax))
            }

            fn syntax(self) -> $crate::cst::SyntaxNode<'a> {
                self.0
            }
        }

        impl<'a> $name<'a> {
            pub fn cast(syntax: $crate::cst::SyntaxNode<'a>) -> Option<Self> {
                <Self as $crate::cst::AstNode<'a>>::cast(syntax)
            }

            pub fn syntax(self) -> $crate::cst::SyntaxNode<'a> {
                self.0
            }

            pub fn span(self) -> $crate::cst::Span {
                self.0.span()
            }
        }
    };
}

macro_rules! debug_node {
    ($name:ident { $($field:ident),* $(,)? }) => {
        debug_node!($name => stringify!($name), { $($field),* });
    };
    ($name:ident => $debug_name:expr, { $($field:ident),* $(,)? }) => {
        impl std::fmt::Debug for $name<'_> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let mut debug = f.debug_struct($debug_name);
                $(debug.field(stringify!($field), &self.$field());)*
                debug.finish()
            }
        }
    };
    ($name:ident => $debug_name:literal { $($field:ident),* $(,)? }) => {
        debug_node!($name => $debug_name, { $($field),* });
    };
}

macro_rules! debug_tuple {
    ($name:ident => $debug_name:literal { $($field:ident),* $(,)? }) => {
        impl std::fmt::Debug for $name<'_> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                let mut debug = f.debug_tuple($debug_name);
                $(debug.field(&self.$field());)*
                debug.finish()
            }
        }
    };
}

pub struct DebugList<I>(pub I);

impl<I> fmt::Debug for DebugList<I>
where
    I: Iterator + Clone,
    I::Item: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.0.clone()).finish()
    }
}

pub struct DebugWith<F>(pub F);

impl<F> fmt::Debug for DebugWith<F>
where
    F: Fn(&mut fmt::Formatter<'_>) -> fmt::Result,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (self.0)(f)
    }
}
