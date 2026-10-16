use std::collections::HashSet;

use falcon_syntax::ast::*;
use falcon_syntax::visitor::bound_names;
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

    pub(crate) fn params(&mut self, params: &FormalParamList) {
        for param in &params.params {
            self.declare(&param.name.name);
        }
    }

    pub(crate) fn pattern(&mut self, pattern: &Pattern) {
        for name in bound_names(pattern) {
            self.declare(&name.name);
        }
    }
}
/// The headers of collection-`for` elements of a literal `node` (looking
/// through nested `if`/`for` elements, which are not expressions).
pub(crate) fn collection_for_inits(node: &Expr) -> Vec<&ForInit> {
    let mut out = Vec::new();
    fn collection_element<'a>(element: &'a CollectionElement, out: &mut Vec<&'a ForInit>) {
        match element {
            CollectionElement::For {
                header, element, ..
            } => {
                out.extend(header.init.as_ref());
                collection_element(element, out);
            }
            CollectionElement::If {
                then_elem,
                else_elem,
                ..
            } => {
                collection_element(then_elem, out);
                if let Some(else_elem) = else_elem {
                    collection_element(else_elem, out);
                }
            }
            CollectionElement::Expr(_)
            | CollectionElement::NullAware { .. }
            | CollectionElement::Spread { .. } => {}
        }
    }
    fn map_element<'a>(element: &'a MapElement, out: &mut Vec<&'a ForInit>) {
        match element {
            MapElement::For { header, entry, .. } => {
                out.extend(header.init.as_ref());
                map_element(entry, out);
            }
            MapElement::If {
                then_entry,
                else_entry,
                ..
            } => {
                map_element(then_entry, out);
                if let Some(else_entry) = else_entry {
                    map_element(else_entry, out);
                }
            }
            MapElement::Entry(_) | MapElement::Spread { .. } => {}
        }
    }
    if let Expr::List { elements, .. } | Expr::Set { elements, .. } = node.unparenthesized() {
        for element in elements {
            collection_element(element, &mut out);
        }
    } else if let Expr::Map { elements, .. } = node.unparenthesized() {
        for element in elements {
            map_element(element, &mut out);
        }
    }
    out
}
