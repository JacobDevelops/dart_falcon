#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClassRestriction {
    Base,
    Interface,
    Final,
    Sealed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ClassModifiers {
    pub is_abstract: bool,
    pub restriction: Option<ClassRestriction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VarKeyword {
    /// Mutable declarations and bindings, with or without an explicit `var`.
    Var,
    Final,
    Const,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsyncMarker {
    Sync,
    SyncStar,
    Async,
    AsyncStar,
}

impl AsyncMarker {
    pub const fn is_async(self) -> bool {
        matches!(self, Self::Async | Self::AsyncStar)
    }

    pub const fn is_generator(self) -> bool {
        matches!(self, Self::SyncStar | Self::AsyncStar)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ParamKind {
    Positional,
    OptionalPositional,
    Named { required: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitializingFormal {
    This,
    Super,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Minus,
    Bang,
    Tilde,
    PlusPlus,
    MinusMinus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostfixIncDec {
    Increment,
    Decrement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    IntDiv,
    EqEq,
    NotEq,
    Lt,
    Gt,
    LtEq,
    GtEq,
    And,
    Or,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    UShr,
    NullCoalesce,
}

impl BinaryOp {
    /// The name of the user-definable operator method this operator invokes;
    /// `None` for `&&`, `||` and `??`. `!=` invokes `==`.
    pub fn method_name(self) -> Option<&'static str> {
        Some(match self {
            Self::Add => "+",
            Self::Sub => "-",
            Self::Mul => "*",
            Self::Div => "/",
            Self::Mod => "%",
            Self::IntDiv => "~/",
            Self::EqEq | Self::NotEq => "==",
            Self::Lt => "<",
            Self::Gt => ">",
            Self::LtEq => "<=",
            Self::GtEq => ">=",
            Self::BitAnd => "&",
            Self::BitOr => "|",
            Self::BitXor => "^",
            Self::Shl => "<<",
            Self::Shr => ">>",
            Self::UShr => ">>>",
            Self::And | Self::Or | Self::NullCoalesce => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssignOp {
    Eq,
    PlusEq,
    MinusEq,
    MulEq,
    DivEq,
    ModEq,
    IntDivEq,
    AndEq,
    OrEq,
    XorEq,
    ShlEq,
    ShrEq,
    UShrEq,
    NullCoalesceEq,
}

impl AssignOp {
    /// The binary operator a compound assignment applies; `None` for `=`.
    pub fn binary(self) -> Option<BinaryOp> {
        Some(match self {
            Self::Eq => return None,
            Self::PlusEq => BinaryOp::Add,
            Self::MinusEq => BinaryOp::Sub,
            Self::MulEq => BinaryOp::Mul,
            Self::DivEq => BinaryOp::Div,
            Self::ModEq => BinaryOp::Mod,
            Self::IntDivEq => BinaryOp::IntDiv,
            Self::AndEq => BinaryOp::BitAnd,
            Self::OrEq => BinaryOp::BitOr,
            Self::XorEq => BinaryOp::BitXor,
            Self::ShlEq => BinaryOp::Shl,
            Self::ShrEq => BinaryOp::Shr,
            Self::UShrEq => BinaryOp::UShr,
            Self::NullCoalesceEq => BinaryOp::NullCoalesce,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelationalPatternOp {
    Eq,
    NotEq,
    Lt,
    Gt,
    LtEq,
    GtEq,
}

/// Delimiter and undecoded content of one string literal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StringDelimiter<'a> {
    pub is_raw: bool,
    pub quote: u8,
    pub triple: bool,
    pub content: &'a str,
    pub content_offset: usize,
    pub terminated: bool,
}

/// Split one source string literal (`r`, quotes, content) into its parts.
pub fn parse_string_delimiter(raw: &str) -> Option<StringDelimiter<'_>> {
    let is_raw = raw.starts_with('r');
    let prefix = usize::from(is_raw);
    let rest = &raw[prefix..];
    let quote = *rest.as_bytes().first()?;
    if !matches!(quote, b'\'' | b'"') {
        return None;
    }
    let triple = rest.as_bytes().get(..3) == Some(&[quote; 3]);
    let len = if triple { 3 } else { 1 };
    let closing = &rest[..len];
    let terminated = rest.len() >= 2 * len && rest[len..].ends_with(closing);
    let content_offset = prefix + len;
    let content_end = if rest.ends_with(closing) {
        (raw.len() - len).max(content_offset)
    } else {
        raw.len()
    };
    Some(StringDelimiter {
        is_raw,
        quote,
        triple,
        content: &raw[content_offset..content_end],
        content_offset,
        terminated,
    })
}
