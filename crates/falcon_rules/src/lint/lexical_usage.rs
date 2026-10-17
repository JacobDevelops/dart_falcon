use std::collections::HashSet;

use falcon_syntax::cst::visit::bound_names;
use falcon_syntax::cst::*;

pub(crate) struct NameScopes(Vec<HashSet<String>>);

impl Default for NameScopes {
    fn default() -> Self {
        Self(vec![HashSet::new()])
    }
}

impl NameScopes {
    pub(crate) fn push(&mut self) {
        self.0.push(HashSet::new());
    }

    pub(crate) fn pop(&mut self) {
        self.0.pop();
    }

    pub(crate) fn declare(&mut self, name: &str) {
        self.0
            .last_mut()
            .expect("lexical scope")
            .insert(name.to_string());
    }

    pub(crate) fn bound(&self, name: &str) -> bool {
        self.0.iter().rev().any(|scope| scope.contains(name))
    }

    pub(crate) fn params(&mut self, params: FormalParamList<'_>) {
        for param in params.params() {
            self.declare(param.name().name());
        }
    }

    pub(crate) fn pattern(&mut self, pattern: Pattern<'_>) {
        for name in bound_names(pattern) {
            self.declare(name.name());
        }
    }
}
/// The headers of collection-`for` elements of a literal `node` (looking
/// through nested `if`/`for` elements, which are not expressions).
pub(crate) fn collection_for_inits<'a>(node: Expr<'a>) -> Vec<ForInit<'a>> {
    let mut out = Vec::new();
    fn collection_element<'a>(element: CollectionElement<'a>, out: &mut Vec<ForInit<'a>>) {
        match element {
            CollectionElement::For(node) => {
                out.extend(node.header().init());
                collection_element(node.element(), out);
            }
            CollectionElement::If(node) => {
                collection_element(node.then_elem(), out);
                if let Some(else_elem) = node.else_elem() {
                    collection_element(else_elem, out);
                }
            }
            CollectionElement::Expr(_)
            | CollectionElement::NullAware(_)
            | CollectionElement::Spread(_) => {}
        }
    }
    fn map_element<'a>(element: MapElement<'a>, out: &mut Vec<ForInit<'a>>) {
        match element {
            MapElement::For(node) => {
                out.extend(node.header().init());
                map_element(node.entry(), out);
            }
            MapElement::If(node) => {
                map_element(node.then_entry(), out);
                if let Some(else_entry) = node.else_entry() {
                    map_element(else_entry, out);
                }
            }
            MapElement::Entry(_) | MapElement::Spread(_) => {}
        }
    }
    match node.unparenthesized() {
        Expr::List(node) => {
            for element in node.elements() {
                collection_element(element, &mut out);
            }
        }
        Expr::Set(node) => {
            for element in node.elements() {
                collection_element(element, &mut out);
            }
        }
        Expr::Map(node) => {
            for element in node.elements() {
                map_element(element, &mut out);
            }
        }
        _ => {}
    }
    out
}
