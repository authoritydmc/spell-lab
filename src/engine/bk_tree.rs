//! BK-tree over Levenshtein distance. The trie is the first candidate
//! generator; the BK-tree is the fallback when the trie search is truncated or
//! when we want an independent edit-distance neighbourhood.

use std::sync::Arc;

use super::distance::levenshtein;

const NONE: u32 = u32::MAX;

#[derive(Debug, Clone, Copy, Default)]
struct Node {
    word_id: u32,
    head: u32,
}

#[derive(Debug, Clone, Copy)]
struct Edge {
    distance: u32,
    child: u32,
    next: u32,
}

#[derive(Debug, Default, Clone)]
pub struct BkTree {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    root: Option<u32>,
}

impl BkTree {
    pub fn build(words: &[Arc<str>]) -> Self {
        let mut tree = Self::default();
        tree.nodes.reserve(words.len());
        tree.edges.reserve(words.len());
        for (word_id, word) in words.iter().enumerate() {
            tree.insert(word_id as u32, word, words);
        }
        tree
    }

    fn insert(&mut self, word_id: u32, word: &str, words: &[Arc<str>]) {
        let Some(root) = self.root else {
            self.nodes.push(Node {
                word_id,
                head: NONE,
            });
            self.root = Some(0);
            return;
        };

        let mut current = root;
        loop {
            let node = self.nodes[current as usize];
            let distance = levenshtein(word, &words[node.word_id as usize]) as u32;
            let mut edge = node.head;
            let mut found = None;
            while edge != NONE {
                let e = self.edges[edge as usize];
                if e.distance == distance {
                    found = Some(e.child);
                    break;
                }
                edge = e.next;
            }
            if let Some(child) = found {
                current = child;
                continue;
            }

            let child = self.nodes.len() as u32;
            self.nodes.push(Node {
                word_id,
                head: NONE,
            });
            let edge_index = self.edges.len() as u32;
            self.edges.push(Edge {
                distance,
                child,
                next: self.nodes[current as usize].head,
            });
            self.nodes[current as usize].head = edge_index;
            return;
        }
    }

    pub fn search(
        &self,
        query: &str,
        words: &[Arc<str>],
        max_distance: usize,
        max_results: usize,
    ) -> Vec<(u32, usize)> {
        let Some(root) = self.root else {
            return Vec::new();
        };
        if max_results == 0 {
            return Vec::new();
        }

        let mut results = Vec::with_capacity(max_results.min(64));
        let mut stack = vec![root];
        while let Some(node_id) = stack.pop() {
            let node = self.nodes[node_id as usize];
            let distance = levenshtein(query, &words[node.word_id as usize]);
            if distance <= max_distance {
                results.push((node.word_id, distance));
                if results.len() >= max_results {
                    break;
                }
            }

            let mut edge = node.head;
            while edge != NONE {
                let e = self.edges[edge as usize];
                let delta = (e.distance as isize - distance as isize).unsigned_abs();
                if delta <= max_distance {
                    stack.push(e.child);
                }
                edge = e.next;
            }
        }
        results
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(values: &[&str]) -> Vec<Arc<str>> {
        values.iter().map(|w| Arc::<str>::from(*w)).collect()
    }

    #[test]
    fn finds_neighbours() {
        let words = words(&["cat", "catalog", "dog", "the", "theatre", "order"]);
        let tree = BkTree::build(&words);
        let results = tree.search("cta", &words, 2, 10);
        let mut found: Vec<_> = results
            .into_iter()
            .map(|(id, _)| words[id as usize].to_string())
            .collect();
        found.sort();
        assert!(found.contains(&"cat".to_string()));

        let results = tree.search("odre", &words, 3, 10);
        let found: Vec<_> = results
            .into_iter()
            .map(|(id, _)| words[id as usize].to_string())
            .collect();
        assert!(found.contains(&"order".to_string()));
    }
}
