//! Cycle-safe ancestor traversal with completeness tracked independently of matches.

use std::collections::HashSet;
use std::hash::Hash;
use std::ops::ControlFlow;

/// An unknown branch prevents a complete negative, but never a later match.
pub(super) fn walk<N: Copy + Eq + Hash, T>(
    root: N,
    mut visit: impl FnMut(N, &mut Vec<N>) -> ControlFlow<T, bool>,
) -> ControlFlow<T, bool> {
    let mut visited = HashSet::new();
    let mut stack = vec![root];
    let mut complete = true;
    while let Some(node) = stack.pop() {
        if visited.insert(node) {
            complete &= visit(node, &mut stack)?;
        }
    }
    ControlFlow::Continue(complete)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_branch_does_not_mask_later_match() {
        let mut visited = Vec::new();
        let result = walk(0, |node, stack| {
            visited.push(node);
            match node {
                0 => stack.extend([1, 2]),
                1 => return ControlFlow::Break(node),
                2 => return ControlFlow::Continue(false),
                _ => unreachable!(),
            }
            ControlFlow::Continue(true)
        });
        assert_eq!(result, ControlFlow::Break(1));
        assert_eq!(visited, [0, 2, 1]);
    }

    #[test]
    fn complete_negative_visits_shared_and_cyclic_paths_once() {
        let mut visited = Vec::new();
        let result: ControlFlow<(), bool> = walk(0, |node, stack| {
            visited.push(node);
            match node {
                0 => stack.extend([1, 2]),
                1 => stack.push(3),
                2 => stack.extend([0, 3]),
                3 => stack.push(2),
                _ => unreachable!(),
            }
            ControlFlow::Continue(true)
        });
        assert_eq!(result, ControlFlow::Continue(true));
        assert_eq!(visited, [0, 2, 3, 1]);
    }

    #[test]
    fn incomplete_branch_prevents_complete_negative() {
        let result: ControlFlow<(), bool> = walk(0, |node, stack| {
            if node == 0 {
                stack.extend([1, 2]);
            }
            ControlFlow::Continue(node != 2)
        });
        assert_eq!(result, ControlFlow::Continue(false));
    }

    #[test]
    fn match_stops_before_pending_branches() {
        let mut visited = Vec::new();
        let result = walk(0, |node, stack| {
            visited.push(node);
            if node == 0 {
                stack.extend([1, 2]);
                ControlFlow::Continue(true)
            } else {
                assert_eq!(node, 2);
                ControlFlow::Break(node)
            }
        });
        assert_eq!(result, ControlFlow::Break(2));
        assert_eq!(visited, [0, 2]);
    }
}
