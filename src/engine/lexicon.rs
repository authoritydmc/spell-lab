use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use anyhow::Result;

use super::{
    bk_tree::BkTree,
    bloom::BloomFilter,
    distance::{damerau_levenshtein, weighted_damerau_levenshtein},
    keyboard,
    trie::Trie,
};

const WORDS_ALPHA: &str = include_str!("../../data/words_alpha.txt");
const EN_50K: &str = include_str!("../../data/en_50k.txt");
const EXTRA_WORDS: &str = include_str!("../../data/extra_words.txt");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuggestionSource {
    Global,
    User,
}

impl SuggestionSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::User => "user",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Suggestion {
    pub word: String,
    pub distance: usize,
    pub confidence: u8,
    pub frequency: u32,
    pub source: SuggestionSource,
    pub reason: &'static str,
}

#[derive(Debug, Clone)]
pub struct Completion {
    pub word: String,
    pub frequency: u32,
    pub source: SuggestionSource,
}

#[derive(Debug)]
pub struct Lexicon {
    words: Vec<Arc<str>>,
    frequencies: Vec<u32>,
    max_frequency: u32,
    frequency_count: usize,
    bloom: BloomFilter,
    trie: Trie,
    bk_tree: BkTree,
    is_user: bool,
}

impl Lexicon {
    pub fn from_entries(mut entries: Vec<(String, u32)>, is_user: bool) -> Self {
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        let mut deduped: Vec<(String, u32)> = Vec::with_capacity(entries.len());
        for (word, freq) in entries {
            if let Some(last) = deduped.last_mut() {
                if last.0 == word {
                    last.1 = last.1.max(freq);
                    continue;
                }
            }
            deduped.push((word, freq));
        }

        let max_frequency = deduped.iter().map(|(_, f)| *f).max().unwrap_or(0);
        let frequency_count = deduped.iter().filter(|(_, f)| *f > 0).count();
        let words: Vec<Arc<str>> = deduped
            .iter()
            .map(|(word, _)| Arc::<str>::from(word.as_str()))
            .collect();
        let frequencies: Vec<u32> = deduped.iter().map(|(_, f)| *f).collect();
        let mut bloom = BloomFilter::with_capacity(words.len(), 0.01);
        for word in &words {
            bloom.insert(word);
        }

        let trie = Trie::build(&words);
        let bk_tree = BkTree::build(&words);

        Self {
            words,
            frequencies,
            max_frequency,
            frequency_count,
            bloom,
            trie,
            bk_tree,
            is_user,
        }
    }

    pub fn load_global() -> Result<Self> {
        let mut frequencies: HashMap<String, u32> = HashMap::new();
        for line in EN_50K.lines() {
            let mut parts = line.split_whitespace();
            let Some(word) = parts.next() else { continue };
            let Some(count) = parts.next().and_then(|v| v.parse::<u32>().ok()) else {
                continue;
            };
            let word = word.to_lowercase();
            if is_lexicon_word(&word) {
                frequencies
                    .entry(word)
                    .and_modify(|current| *current = (*current).max(count))
                    .or_insert(count);
            }
        }

        let mut entries: Vec<(String, u32)> = Vec::with_capacity(380_000);
        for line in WORDS_ALPHA.lines() {
            let word = line.trim().to_lowercase();
            if is_lexicon_word(&word) {
                let frequency = frequencies.get(&word).copied().unwrap_or(0);
                entries.push((word, frequency));
            }
        }
        for line in EXTRA_WORDS.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let word = line.to_lowercase();
            if is_lexicon_word(&word) {
                let frequency = frequencies.get(&word).copied().unwrap_or(0);
                entries.push((word, frequency));
            }
        }
        // Include high-frequency words that are missing from words_alpha
        // (some contractions / modern tokens).
        for (word, frequency) in frequencies {
            if is_lexicon_word(&word) {
                entries.push((word, frequency));
            }
        }

        Ok(Self::from_entries(entries, false))
    }

    pub fn len(&self) -> usize {
        self.words.len()
    }

    pub fn frequency_count(&self) -> usize {
        self.frequency_count
    }

    pub fn trie_nodes(&self) -> usize {
        self.trie.node_count()
    }

    pub fn bk_nodes(&self) -> usize {
        self.bk_tree.node_count()
    }

    pub fn contains(&self, word: &str) -> bool {
        if !self.bloom.contains(word) {
            return false;
        }
        self.trie.contains(word)
    }

    pub fn completions(&self, prefix: &str, limit: usize) -> Vec<Completion> {
        if prefix.is_empty() || limit == 0 || !prefix.is_ascii() {
            return Vec::new();
        }
        let ids = self
            .trie
            .prefix_completions(prefix, limit.saturating_mul(20).max(50));
        let mut out: Vec<Completion> = ids
            .into_iter()
            .map(|id| Completion {
                word: self.words[id as usize].to_string(),
                frequency: self.frequencies[id as usize],
                source: self.source(),
            })
            .collect();
        out.sort_by(|a, b| {
            b.frequency
                .cmp(&a.frequency)
                .then_with(|| a.word.len().cmp(&b.word.len()))
                .then_with(|| a.word.cmp(&b.word))
        });
        out.truncate(limit);
        out
    }

    pub fn suggest(
        &self,
        query: &str,
        max_suggestions: usize,
        max_distance: usize,
        min_confidence: f64,
    ) -> Vec<Suggestion> {
        if max_suggestions == 0 || query.is_empty() || !query.is_ascii() {
            return Vec::new();
        }
        let query = query.to_ascii_lowercase();
        if query.chars().count() <= 1 {
            return Vec::new();
        }
        let max_distance = max_distance.clamp(1, 3);
        // Search one extra edit so transpositions (which are 2 Levenshtein
        // edits but 1 Damerau edit) are not lost.
        let search_radius = if max_distance >= 3 {
            3
        } else {
            max_distance + 1
        };

        let mut candidate_ids: HashSet<u32> = HashSet::new();
        let trie_results = self.trie.fuzzy_search(&query, search_radius, 1_500);
        for (id, _) in &trie_results {
            candidate_ids.insert(*id);
        }

        // The BK-tree is an independent fallback, especially useful when the
        // trie automation hit its result/visit budget.
        if trie_results.len() < 40 {
            for (id, _) in self
                .bk_tree
                .search(&query, &self.words, search_radius, 1_000)
            {
                candidate_ids.insert(id);
            }
        }

        if candidate_ids.len() < max_suggestions.saturating_mul(4).max(12) {
            for id in self
                .trie
                .prefix_completions(&query, (max_suggestions * 8).max(80))
            {
                candidate_ids.insert(id);
            }
        }

        let mut suggestions: Vec<Suggestion> = Vec::with_capacity(candidate_ids.len());
        for id in candidate_ids {
            let word = &self.words[id as usize];
            // Two-letter dictionary entries ("th", "te") are rarely useful
            // typo repairs for a three-or-more-letter query and crowd out the
            // intended word.
            if word.chars().count() < 3 && query.chars().count() >= 3 {
                continue;
            }
            let distance = damerau_levenshtein(&query, word);
            if distance > max_distance {
                continue;
            }
            let weighted = weighted_damerau_levenshtein(&query, word);
            let frequency = self.frequencies[id as usize];
            let confidence = confidence(
                &query,
                word,
                distance,
                weighted,
                frequency,
                self.max_frequency,
                self.is_user,
            );
            if confidence as f64 + 1e-9 < min_confidence {
                continue;
            }
            suggestions.push(Suggestion {
                word: word.to_string(),
                distance,
                confidence,
                frequency,
                source: self.source(),
                reason: reason(&query, word),
            });
        }

        suggestions.sort_by(|a, b| {
            b.confidence
                .cmp(&a.confidence)
                .then_with(|| a.distance.cmp(&b.distance))
                .then_with(|| b.frequency.cmp(&a.frequency))
                .then_with(|| {
                    let a_len = a.word.len().abs_diff(query.len());
                    let b_len = b.word.len().abs_diff(query.len());
                    a_len.cmp(&b_len)
                })
                .then_with(|| a.word.cmp(&b.word))
        });
        suggestions.truncate(max_suggestions);
        suggestions
    }

    fn source(&self) -> SuggestionSource {
        if self.is_user {
            SuggestionSource::User
        } else {
            SuggestionSource::Global
        }
    }
}

fn is_lexicon_word(word: &str) -> bool {
    if word.is_empty() || word.chars().count() > 64 {
        return false;
    }
    if !word
        .chars()
        .all(|c| c.is_ascii_alphabetic() || c == '\'' || c == '-')
    {
        return false;
    }
    !word.starts_with('-')
        && !word.ends_with('-')
        && !word.starts_with('\'')
        && !word.ends_with('\'')
        && !word.contains("--")
        && !word.contains("''")
}

#[allow(clippy::too_many_arguments)]
fn confidence(
    query: &str,
    word: &str,
    distance: usize,
    weighted: f64,
    frequency: u32,
    max_frequency: u32,
    is_user: bool,
) -> u8 {
    let distance_score = match distance {
        0 => 1.0,
        1 => 0.84,
        2 => 0.60,
        3 => 0.38,
        _ => 0.20,
    };
    let mut frequency_score = if max_frequency > 0 {
        ((frequency as f64 + 1.0).ln() / (max_frequency as f64 + 1.0).ln()).clamp(0.0, 1.0)
    } else {
        0.0
    };
    if is_user {
        // A curated word should not lose to an obscure global word just
        // because we do not have corpus counts for user dictionaries.
        frequency_score = frequency_score.max(0.55);
    }
    let max_len = query.chars().count().max(word.chars().count()).max(1);
    let length_diff = query.chars().count().abs_diff(word.chars().count());
    let length_score = 1.0 - (length_diff as f64 / max_len as f64) * 0.35;
    let keyboard_bonus = if distance > 0 && weighted < distance as f64 {
        (1.0 - weighted / distance as f64).clamp(0.0, 1.0) * 0.06
    } else {
        0.0
    };
    let source_bonus = if is_user { 0.08 } else { 0.0 };
    // A missed letter plus a nearby transposition is a very common typo
    // ("odre" -> "order"). Reward candidates that contain every query letter
    // plus exactly one extra letter, without letting unrelated words through.
    let misordered_bonus = if distance >= 2
        && query.chars().count() >= 4
        && word.chars().count() == query.chars().count() + 1
        && is_letter_subset(query, word)
    {
        0.10
    } else {
        0.0
    };
    let raw = 0.60 * distance_score
        + 0.27 * frequency_score
        + 0.13 * length_score
        + keyboard_bonus
        + source_bonus
        + misordered_bonus;
    (raw * 100.0).clamp(1.0, 99.0).round() as u8
}

fn is_letter_subset(needle: &str, haystack: &str) -> bool {
    let mut available: Vec<char> = haystack
        .chars()
        .filter(|c| c.is_ascii_alphabetic())
        .map(|c| c.to_ascii_lowercase())
        .collect();
    available.sort_unstable();
    for letter in needle
        .chars()
        .filter(|c| c.is_ascii_alphabetic())
        .map(|c| c.to_ascii_lowercase())
    {
        match available.binary_search(&letter) {
            Ok(index) => {
                available.remove(index);
            }
            Err(_) => return false,
        }
    }
    true
}

fn reason(query: &str, word: &str) -> &'static str {
    if query == word {
        return "exact";
    }
    if query.chars().count() == word.chars().count() {
        let diffs: Vec<(char, char)> = query
            .chars()
            .zip(word.chars())
            .filter(|(a, b)| a != b)
            .collect();
        if diffs.len() == 1 {
            return if keyboard::is_keyboard_neighbour(diffs[0].0, diffs[0].1) {
                "keyboard"
            } else {
                "substitution"
            };
        }
        if diffs.len() == 2 && diffs[0].1 == diffs[1].0 && diffs[0].0 == diffs[1].1 {
            return "transposition";
        }
        let mut a: Vec<char> = query.chars().collect();
        let mut b: Vec<char> = word.chars().collect();
        a.sort_unstable();
        b.sort_unstable();
        if a == b {
            return "anagram";
        }
    }
    if query.chars().count() >= 4
        && word.chars().count() == query.chars().count() + 1
        && is_letter_subset(query, word)
    {
        return "misordered";
    }
    if word.starts_with(query) {
        return "prefix";
    }
    if word.chars().count() > query.chars().count() {
        return "insertion";
    }
    if word.chars().count() < query.chars().count() {
        return "deletion";
    }
    "edit"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_lexicon() -> Lexicon {
        Lexicon::from_entries(
            vec![
                ("the".into(), 1_000_000),
                ("tea".into(), 80_000),
                ("order".into(), 60_000),
                ("other".into(), 50_000),
                ("spell".into(), 20_000),
                ("cat".into(), 10_000),
                ("dog".into(), 8_000),
            ],
            false,
        )
    }

    #[test]
    fn membership_and_completion() {
        let lex = test_lexicon();
        assert!(lex.contains("the"));
        assert!(!lex.contains("teh"));
        let completions = lex.completions("sp", 5);
        assert_eq!(completions[0].word, "spell");
    }

    #[test]
    fn ranks_transposition() {
        let lex = test_lexicon();
        let suggestions = lex.suggest("teh", 5, 2, 0.0);
        assert!(!suggestions.is_empty());
        assert_eq!(suggestions[0].word, "the");
        assert_eq!(suggestions[0].distance, 1);
        assert_eq!(suggestions[0].reason, "transposition");
    }

    #[test]
    fn finds_order_from_misordered() {
        let lex = test_lexicon();
        let suggestions = lex.suggest("odre", 5, 2, 0.0);
        assert!(suggestions.iter().any(|s| s.word == "order"));
    }

    #[test]
    fn user_words_get_a_source_boost() {
        let global = test_lexicon();
        let user = Lexicon::from_entries(vec![("kubernetes".into(), 0)], true);
        assert!(user.suggest("kubernets", 1, 2, 0.0)[0].confidence >= 80);
        assert_eq!(global.suggest("kubernets", 1, 2, 0.0).len(), 0);
    }

    #[test]
    fn confidence_is_bounded() {
        let s = confidence("teh", "the", 1, 0.72, 1_000_000, 1_000_000, false);
        assert!((1..=99).contains(&s));
    }
}
