//! Compact byte trie used for membership, prefix completion and
//! Levenshtein-automaton candidate generation.
//!
//! Children are stored in an append-only edge pool with sibling links, which
//! keeps building 300k+ English words cheap and avoids shifting edge indexes.

use std::sync::Arc;

const NONE: u32 = u32::MAX;

#[derive(Debug, Clone, Copy, Default)]
struct Node {
    head: u32,
    terminal: u32,
}

#[derive(Debug, Clone, Copy)]
struct Edge {
    ch: u8,
    child: u32,
    next: u32,
}

impl Default for Edge {
    fn default() -> Self {
        Self {
            ch: 0,
            child: NONE,
            next: NONE,
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct Trie {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
}

impl Trie {
    pub fn build(words: &[Arc<str>]) -> Self {
        let mut trie = Self::default();
        trie.nodes.push(Node {
            head: NONE,
            terminal: NONE,
        });
        trie.nodes.reserve(words.len().saturating_mul(2));
        trie.edges.reserve(words.len().saturating_mul(2));

        for (word_id, word) in words.iter().enumerate() {
            let mut node = 0u32;
            for byte in word.as_bytes() {
                if let Some(child) = trie.child(node, *byte) {
                    node = child;
                } else {
                    let child = trie.nodes.len() as u32;
                    trie.nodes.push(Node {
                        head: NONE,
                        terminal: NONE,
                    });
                    let edge = trie.edges.len() as u32;
                    trie.edges.push(Edge {
                        ch: *byte,
                        child,
                        next: trie.nodes[node as usize].head,
                    });
                    trie.nodes[node as usize].head = edge;
                    node = child;
                }
            }
            trie.nodes[node as usize].terminal = word_id as u32;
        }
        trie
    }

    fn child(&self, node: u32, byte: u8) -> Option<u32> {
        let mut edge = self.nodes[node as usize].head;
        while edge != NONE {
            let e = self.edges[edge as usize];
            if e.ch == byte {
                return Some(e.child);
            }
            edge = e.next;
        }
        None
    }

    pub fn contains(&self, word: &str) -> bool {
        self.word_id(word).is_some()
    }

    fn word_id(&self, word: &str) -> Option<u32> {
        if self.nodes.is_empty() {
            return None;
        }
        let mut node = 0u32;
        for byte in word.as_bytes() {
            node = self.child(node, *byte)?;
        }
        let terminal = self.nodes[node as usize].terminal;
        (terminal != NONE).then_some(terminal)
    }

    /// Return up to `limit` dictionary word IDs with the given prefix.
    pub fn prefix_completions(&self, prefix: &str, limit: usize) -> Vec<u32> {
        if limit == 0 {
            return Vec::new();
        }
        let mut node = 0u32;
        for byte in prefix.as_bytes() {
            let Some(child) = self.child(node, *byte) else {
                return Vec::new();
            };
            node = child;
        }

        let mut out = Vec::with_capacity(limit.min(64));
        let mut stack = vec![node];
        while let Some(node) = stack.pop() {
            let terminal = self.nodes[node as usize].terminal;
            if terminal != NONE {
                out.push(terminal);
                if out.len() >= limit {
                    break;
                }
            }
            let mut edge = self.nodes[node as usize].head;
            while edge != NONE {
                stack.push(self.edges[edge as usize].child);
                edge = self.edges[edge as usize].next;
            }
        }
        out
    }

    /// Levenshtein-automaton search over the trie. This intentionally uses
    /// plain Levenshtein; transposition candidates are recovered because the
    /// caller searches with a slightly larger radius and then verifies with
    /// Damerau–Levenshtein distance.
    pub fn fuzzy_search(
        &self,
        query: &str,
        max_distance: usize,
        max_results: usize,
    ) -> Vec<(u32, usize)> {
        if self.nodes.is_empty() || query.is_empty() || max_results == 0 || !query.is_ascii() {
            return Vec::new();
        }
        let query = query.as_bytes();
        let mut row: Vec<usize> = (0..=query.len()).collect();
        let mut results = Vec::with_capacity(max_results.min(128));
        let mut visits = 0usize;
        self.fuzzy_dfs(
            0,
            query,
            &mut row,
            max_distance,
            max_results,
            &mut results,
            &mut visits,
        );
        results
    }

    #[allow(clippy::too_many_arguments)]
    fn fuzzy_dfs(
        &self,
        node: u32,
        query: &[u8],
        prev_row: &mut [usize],
        max_distance: usize,
        max_results: usize,
        results: &mut Vec<(u32, usize)>,
        visits: &mut usize,
    ) {
        if results.len() >= max_results || *visits > 500_000 {
            return;
        }
        let node_ref = self.nodes[node as usize];
        let mut edge = node_ref.head;
        while edge != NONE {
            *visits += 1;
            let e = self.edges[edge as usize];
            let mut row = vec![0usize; query.len() + 1];
            row[0] = prev_row[0] + 1;
            for (j, q) in query.iter().enumerate() {
                let cost = usize::from(*q != e.ch);
                row[j + 1] = (row[j] + 1)
                    .min(prev_row[j + 1] + 1)
                    .min(prev_row[j] + cost);
            }
            let child = self.nodes[e.child as usize];
            let distance = row[query.len()];
            if child.terminal != NONE && distance <= max_distance {
                results.push((child.terminal, distance));
                if results.len() >= max_results {
                    return;
                }
            }
            if row.iter().copied().min().unwrap_or(usize::MAX) <= max_distance {
                self.fuzzy_dfs(
                    e.child,
                    query,
                    &mut row,
                    max_distance,
                    max_results,
                    results,
                    visits,
                );
            }
            edge = e.next;
        }
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
    fn membership_and_prefix() {
        let words = words(&["cat", "catalog", "dog", "the", "theatre"]);
        let trie = Trie::build(&words);
        assert!(trie.contains("cat"));
        assert!(trie.contains("theatre"));
        assert!(!trie.contains("ca"));
        assert!(!trie.contains("cats"));

        let mut completions: Vec<_> = trie
            .prefix_completions("the", 10)
            .into_iter()
            .map(|id| words[id as usize].to_string())
            .collect();
        completions.sort();
        assert_eq!(completions, vec!["the", "theatre"]);

        let mut cat: Vec<_> = trie
            .prefix_completions("cat", 10)
            .into_iter()
            .map(|id| words[id as usize].to_string())
            .collect();
        cat.sort();
        assert_eq!(cat, vec!["cat", "catalog"]);
    }

    #[test]
    fn fuzzy_finds_one_edit() {
        let words = words(&["cat", "catalog", "dog", "the", "theatre"]);
        let trie = Trie::build(&words);
        let results = trie.fuzzy_search("cta", 2, 10);
        let mut found: Vec<_> = results
            .into_iter()
            .map(|(id, _)| words[id as usize].to_string())
            .collect();
        found.sort();
        assert!(found.contains(&"cat".to_string()));
    }
}
