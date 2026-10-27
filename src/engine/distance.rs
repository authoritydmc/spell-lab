use crate::engine::keyboard;

/// Classic Levenshtein distance. This is the metric used by the BK-tree.
pub fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.is_empty() {
        return b.len();
    }
    if b.is_empty() {
        return a.len();
    }

    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut curr = vec![0usize; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        curr[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            curr[j + 1] = (prev[j + 1] + 1).min(curr[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b.len()]
}

/// Optimal-string-alignment Damerau–Levenshtein distance. Adjacent
/// transpositions cost one edit, so `teh -> the` is distance 1.
pub fn damerau_levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.is_empty() {
        return b.len();
    }
    if b.is_empty() {
        return a.len();
    }

    // Three rolling rows: i-2, i-1, i.
    let mut row2 = vec![0usize; b.len() + 1];
    let mut row1: Vec<usize> = (0..=b.len()).collect();
    let mut row0 = vec![0usize; b.len() + 1];

    for i in 1..=a.len() {
        row0[0] = i;
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            let mut value = (row1[j - 1] + cost).min(row1[j] + 1).min(row0[j - 1] + 1);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                value = value.min(row2[j - 2] + 1);
            }
            row0[j] = value;
        }
        std::mem::swap(&mut row2, &mut row1);
        std::mem::swap(&mut row1, &mut row0);
    }
    row1[b.len()]
}

/// Weighted edit distance used only for ranking. Adjacent transpositions and
/// keyboard-neighbour substitutions are cheaper than arbitrary edits.
pub fn weighted_damerau_levenshtein(a: &str, b: &str) -> f64 {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.is_empty() {
        return b.len() as f64;
    }
    if b.is_empty() {
        return a.len() as f64;
    }

    let mut row2 = vec![0.0f64; b.len() + 1];
    let mut row1: Vec<f64> = (0..=b.len()).map(|v| v as f64).collect();
    let mut row0 = vec![0.0f64; b.len() + 1];

    for i in 1..=a.len() {
        row0[0] = i as f64;
        for j in 1..=b.len() {
            let substitution = keyboard::substitution_cost(a[i - 1], b[j - 1]);
            let mut value = (row1[j - 1] + substitution)
                .min(row1[j] + 1.0)
                .min(row0[j - 1] + 1.0);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                value = value.min(row2[j - 2] + 0.72);
            }
            row0[j] = value;
        }
        std::mem::swap(&mut row2, &mut row1);
        std::mem::swap(&mut row1, &mut row0);
    }
    row1[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distances() {
        assert_eq!(levenshtein("kitten", "sitting"), 3);
        assert_eq!(levenshtein("teh", "the"), 2);
        assert_eq!(damerau_levenshtein("teh", "the"), 1);
        assert_eq!(damerau_levenshtein("odre", "order"), 2);
        assert_eq!(damerau_levenshtein("", "abc"), 3);
        assert_eq!(damerau_levenshtein("abc", "abc"), 0);
        let weighted = weighted_damerau_levenshtein("teh", "the");
        assert!(weighted < 1.0, "weighted distance was {weighted}");
        assert!(weighted_damerau_levenshtein("cat", "dog") > 2.0);
    }
}
