use std::sync::Arc;

use parking_lot::RwLock;
use petgraph::{
    graph::{DiGraph, NodeIndex},
    Direction,
};
use spliter::Spliterator;

use crate::dependency_graph_store::ModuleID;

type Graph = DiGraph<ModuleID, ModuleID>;

pub struct DepthFirstExpansion<'a> {
    direction: Direction,
    graph: &'a Graph,
    stack: Vec<NodeIndex>,
    pub seen_nodes: Arc<RwLock<Vec<bool>>>,
}

impl<'a> DepthFirstExpansion<'a> {
    /// Create a new search with the given starting point.
    pub fn new(graph: &'a Graph, direction: Direction, node_idx: NodeIndex) -> Self {
        return Self {
            direction,
            graph,
            seen_nodes: Arc::new(RwLock::new(vec![false; graph.node_count()])),
            stack: vec![node_idx],
        };
    }
}

impl<'a> Iterator for DepthFirstExpansion<'a> {
    type Item = NodeIndex;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(node_idx) = self.stack.pop() {
            if self.seen_nodes.read()[node_idx.index()] {
                return Some(node_idx);
            }
            self.seen_nodes.write().insert(node_idx.index(), true);

            self.stack
                .extend(self.graph.neighbors_directed(node_idx, self.direction));
            return Some(node_idx);
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
                seen_nodes: self.seen_nodes.clone(),
                stack,
            });
        } else {
            return None;
        }
    }
}
