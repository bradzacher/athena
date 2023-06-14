use std::sync::Arc;

use parking_lot::RwLock;
use petgraph::{
    graph::{DiGraph, NodeIndex},
    Direction,
};
use spliter::Spliterator;

use crate::module::ModuleId;

type Graph = DiGraph<ModuleId, ModuleId>;

struct StackItem {
    node_idx: NodeIndex,
    depth: u32,
}

pub struct DepthFirstExpansion<'a> {
    direction: Direction,
    graph: &'a Graph,
    max_depth: u32,
    stack: Vec<StackItem>,
    seen_nodes: Arc<RwLock<Vec<bool>>>,
}

impl<'a> DepthFirstExpansion<'a> {
    /// Create a new search with the given starting point.
    pub fn new(
        graph: &'a Graph,
        direction: Direction,
        max_depth: u32,
        node_idx: NodeIndex,
    ) -> Self {
        return Self {
            direction,
            graph,
            max_depth,
            seen_nodes: Arc::new(RwLock::new(vec![false; graph.node_count()])),
            stack: vec![StackItem { node_idx, depth: 0 }],
        };
    }
}

impl<'a> Iterator for DepthFirstExpansion<'a> {
    type Item = NodeIndex;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(item) = self.stack.pop() {
            if self.seen_nodes.read()[item.node_idx.index()] {
                // already seen - don't expand
                return Some(item.node_idx);
            }
            self.seen_nodes.write().insert(item.node_idx.index(), true);

            if self.max_depth > 0 && item.depth >= self.max_depth {
                // hit max depth - don't expand further
                return Some(item.node_idx);
            }

            let new_depth = item.depth + 1;
            self.stack.extend(
                self.graph
                    .neighbors_directed(item.node_idx, self.direction)
                    .map(|neighbor| StackItem {
                        node_idx: neighbor,
                        depth: new_depth,
                    }),
            );
            return Some(item.node_idx);
        }

        // the None return tells the iterator the iteration is finsihed to exit
        return None;
    }
}

impl<'a> Spliterator for DepthFirstExpansion<'a> {
    /// Split this traversal in half if possible.
    fn split(&mut self) -> Option<Self> {
        let len = self.stack.len();
        if len >= 2 {
            let stack = self.stack.split_off(len / 2);
            return Some(Self {
                direction: self.direction,
                graph: &self.graph,
                max_depth: self.max_depth,
                seen_nodes: self.seen_nodes.clone(),
                stack,
            });
        } else {
            return None;
        }
    }
}
