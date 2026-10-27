use std::{collections::HashMap, sync::Arc, time::Instant};

use axum::{
    extract::{Query, State},
    http::HeaderMap,
    Json,
};

use crate::{
    engine::{tokenize, Lexicon, Suggestion, SuggestionSource},
    error::AppError,
    models::{
        CheckRequest, CheckResponse, CheckStats, CompleteQuery, CompleteResponse, CompletionView,
        CorrectRequest, CorrectResponse, CorrectionChange, DictionarySnapshot, SuggestRequest,
        SuggestResponse, SuggestionView, TokenView,
    },
    state::AppState,
};

#[derive(Debug, Clone, Copy)]
struct Limits {
    max_suggestions: usize,
    max_distance: usize,
    min_confidence: f64,
}

pub async fn check(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CheckRequest>,
) -> Result<Json<CheckResponse>, AppError> {
    let started = Instant::now();
    let limits = resolve_limits(
        &state,
        request.max_suggestions,
        request.max_distance,
        request.min_confidence,
    );
    validate_text(&state, &request.text)?;

    let user_lexicon = load_user_lexicon(&state, &headers, request.include_user_dictionary).await?;
    let tokens = tokenize(&request.text);
    let text_chars = request.text.chars().count();
    let mut views = Vec::with_capacity(tokens.len());
    let mut correct_words = 0usize;
    let mut misspelled_words = 0usize;
    let mut ignored_words = 0usize;
    let mut suggestion_count = 0usize;

    for token in tokens {
        let normalized = token.normalized.as_str();
        let in_global = is_known(&state.global, normalized);
        let in_user = user_lexicon
            .as_ref()
            .map(|lexicon| is_known(lexicon, normalized))
            .unwrap_or(false);
        let ignored = (request.ignore_acronyms && token.acronym) || !normalized.is_ascii();
        let correct = ignored || in_global || in_user;

        let suggestions = if correct || limits.max_suggestions == 0 {
            Vec::new()
        } else {
            combined_suggestions(&state, user_lexicon.as_deref(), normalized, limits)
        };
        suggestion_count += suggestions.len();
        if correct {
            if ignored {
                ignored_words += 1;
            } else {
                correct_words += 1;
            }
        } else {
            misspelled_words += 1;
        }

        views.push(TokenView {
            raw: token.raw.clone(),
            normalized: token.normalized,
            start: token.start_char,
            end: token.end_char,
            byte_start: token.start_byte,
            byte_end: token.end_byte,
            correct,
            ignored,
            in_user_dictionary: in_user,
            suggestions: suggestions
                .iter()
                .map(|suggestion| suggestion_view(&token.raw, suggestion))
                .collect(),
        });
    }

    let elapsed_ms = round_ms(started.elapsed().as_secs_f64() * 1000.0);
    Ok(Json(CheckResponse {
        text: request.text,
        stats: CheckStats {
            total_tokens: views.len(),
            correct_words,
            misspelled_words,
            ignored_words,
            suggestion_count,
            text_chars,
            elapsed_ms,
        },
        dictionary: DictionarySnapshot {
            global_words: state.global.len(),
            user_words: user_lexicon.as_ref().map(|l| l.len()).unwrap_or(0),
            user_dictionary_included: user_lexicon.is_some(),
        },
        tokens: views,
    }))
}

pub async fn suggest_word(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<SuggestRequest>,
) -> Result<Json<SuggestResponse>, AppError> {
    let started = Instant::now();
    let limits = resolve_limits(
        &state,
        request.max_suggestions,
        request.max_distance,
        request.min_confidence,
    );
    let word = normalize_query(&request.word)?;
    let user_lexicon = load_user_lexicon(&state, &headers, request.include_user_dictionary).await?;
    let in_global = is_known(&state.global, &word);
    let in_user = user_lexicon
        .as_ref()
        .map(|lexicon| is_known(lexicon, &word))
        .unwrap_or(false);
    let correct = in_global || in_user;
    let suggestions = if correct || limits.max_suggestions == 0 {
        Vec::new()
    } else {
        combined_suggestions(&state, user_lexicon.as_deref(), &word, limits)
    };

    Ok(Json(SuggestResponse {
        word: request.word,
        correct,
        user_dictionary_included: user_lexicon.is_some(),
        suggestions: suggestions
            .iter()
            .map(|suggestion| suggestion_view(&word, suggestion))
            .collect(),
        elapsed_ms: round_ms(started.elapsed().as_secs_f64() * 1000.0),
    }))
}

pub async fn complete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<CompleteQuery>,
) -> Result<Json<CompleteResponse>, AppError> {
    let prefix = query.prefix.trim().to_lowercase();
    if prefix.is_empty() || prefix.chars().count() > 64 || !is_query_word(&prefix) {
        return Err(AppError::bad_request(
            "prefix must be 1-64 ASCII letters, ' or -",
        ));
    }
    let limit = query.limit.unwrap_or(10).clamp(1, 50);
    let user_lexicon = load_user_lexicon(&state, &headers, query.include_user_dictionary).await?;

    let mut merged: HashMap<String, CompletionView> = HashMap::new();
    for completion in state.global.completions(&prefix, limit.saturating_mul(2)) {
        merged.insert(
            completion.word.clone(),
            CompletionView {
                word: completion.word,
                frequency: completion.frequency,
                source: completion.source.as_str().to_string(),
            },
        );
    }
    if let Some(user_lexicon) = user_lexicon.as_deref() {
        for completion in user_lexicon.completions(&prefix, limit.saturating_mul(2)) {
            merged
                .entry(completion.word.clone())
                .and_modify(|existing| {
                    existing.source = completion.source.as_str().to_string();
                    existing.frequency = existing.frequency.max(completion.frequency);
                })
                .or_insert(CompletionView {
                    word: completion.word,
                    frequency: completion.frequency,
                    source: completion.source.as_str().to_string(),
                });
        }
    }
    let mut completions: Vec<CompletionView> = merged.into_values().collect();
    completions.sort_by(|a, b| {
        b.frequency
            .cmp(&a.frequency)
            .then_with(|| a.word.cmp(&b.word))
    });
    completions.truncate(limit);
    Ok(Json(CompleteResponse {
        prefix,
        completions,
    }))
}

pub async fn correct(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CorrectRequest>,
) -> Result<Json<CorrectResponse>, AppError> {
    let started = Instant::now();
    validate_text(&state, &request.text)?;
    let limits = Limits {
        max_suggestions: 1,
        max_distance: request
            .max_distance
            .unwrap_or(state.config.default_max_distance)
            .clamp(1, state.config.max_distance_limit),
        min_confidence: request.min_confidence.unwrap_or(85.0).clamp(0.0, 100.0),
    };
    let user_lexicon = load_user_lexicon(&state, &headers, request.include_user_dictionary).await?;
    let tokens = tokenize(&request.text);
    let mut corrected = String::with_capacity(request.text.len());
    let mut changes = Vec::new();
    let mut last_byte = 0usize;

    for token in tokens {
        let in_global = is_known(&state.global, &token.normalized);
        let in_user = user_lexicon
            .as_ref()
            .map(|lexicon| is_known(lexicon, &token.normalized))
            .unwrap_or(false);
        let ignored = (request.ignore_acronyms && token.acronym) || !token.normalized.is_ascii();
        if ignored || in_global || in_user {
            continue;
        }
        let suggestions =
            combined_suggestions(&state, user_lexicon.as_deref(), &token.normalized, limits);
        let Some(best) = suggestions.first() else {
            continue;
        };
        let replacement = match_case(&token.raw, &best.word);
        corrected.push_str(&request.text[last_byte..token.start_byte]);
        corrected.push_str(&replacement);
        last_byte = token.end_byte;
        changes.push(CorrectionChange {
            start: token.start_char,
            end: token.end_char,
            original: token.raw,
            replacement,
            confidence: best.confidence,
        });
    }
    corrected.push_str(&request.text[last_byte..]);

    Ok(Json(CorrectResponse {
        original: request.text,
        corrected,
        changes,
        elapsed_ms: round_ms(started.elapsed().as_secs_f64() * 1000.0),
    }))
}

async fn load_user_lexicon(
    state: &AppState,
    headers: &HeaderMap,
    include_user_dictionary: bool,
) -> Result<Option<Arc<Lexicon>>, AppError> {
    if !include_user_dictionary {
        return Ok(None);
    }
    let Some(user) = state.optional_user(headers).await? else {
        return Ok(None);
    };
    state.user_lexicon(&user.id).await.map(Some)
}

fn resolve_limits(
    state: &AppState,
    max_suggestions: Option<usize>,
    max_distance: Option<usize>,
    min_confidence: Option<f64>,
) -> Limits {
    Limits {
        max_suggestions: max_suggestions
            .unwrap_or(state.config.default_max_suggestions)
            .min(state.config.max_suggestions_limit),
        max_distance: max_distance
            .unwrap_or(state.config.default_max_distance)
            .clamp(1, state.config.max_distance_limit),
        min_confidence: min_confidence
            .unwrap_or(state.config.default_min_confidence)
            .clamp(0.0, 100.0),
    }
}

fn combined_suggestions(
    state: &AppState,
    user: Option<&Lexicon>,
    query: &str,
    limits: Limits,
) -> Vec<Suggestion> {
    if limits.max_suggestions == 0 {
        return Vec::new();
    }
    let request_count = (limits.max_suggestions * 2).clamp(1, 20);
    let mut merged: HashMap<String, Suggestion> = HashMap::new();
    for suggestion in state.cached_global_suggestions(
        query,
        limits.max_suggestions,
        limits.max_distance,
        limits.min_confidence,
    ) {
        merged.insert(suggestion.word.clone(), suggestion);
    }
    if let Some(user) = user {
        for suggestion in user.suggest(
            query,
            request_count,
            limits.max_distance,
            limits.min_confidence,
        ) {
            match merged.get(&suggestion.word) {
                Some(existing)
                    if existing.source == SuggestionSource::User
                        || (suggestion.source == SuggestionSource::Global
                            && existing.confidence >= suggestion.confidence) => {}
                _ => {
                    merged.insert(suggestion.word.clone(), suggestion);
                }
            }
        }
    }
    let mut suggestions: Vec<Suggestion> = merged.into_values().collect();
    suggestions.sort_by(|a, b| {
        b.confidence
            .cmp(&a.confidence)
            .then_with(|| a.distance.cmp(&b.distance))
            .then_with(|| b.frequency.cmp(&a.frequency))
            .then_with(|| {
                a.word
                    .len()
                    .abs_diff(query.len())
                    .cmp(&b.word.len().abs_diff(query.len()))
            })
            .then_with(|| a.word.cmp(&b.word))
    });
    suggestions.truncate(limits.max_suggestions);
    suggestions
}

fn suggestion_view(raw: &str, suggestion: &Suggestion) -> SuggestionView {
    SuggestionView {
        word: match_case(raw, &suggestion.word),
        distance: suggestion.distance,
        confidence: suggestion.confidence,
        frequency: suggestion.frequency,
        source: suggestion.source.as_str().to_string(),
        reason: suggestion.reason.to_string(),
    }
}

fn is_known(lexicon: &Lexicon, word: &str) -> bool {
    if lexicon.contains(word) {
        return true;
    }
    if let Some(base) = word.strip_suffix("'s") {
        if !base.is_empty() && lexicon.contains(base) {
            return true;
        }
    }
    if let Some(base) = word.strip_suffix('\'') {
        if !base.is_empty() && lexicon.contains(base) {
            return true;
        }
    }
    if word.contains('-') {
        return word
            .split('-')
            .filter(|part| !part.is_empty())
            .all(|part| lexicon.contains(part));
    }
    false
}

fn match_case(raw: &str, suggestion: &str) -> String {
    let letters: Vec<char> = raw.chars().filter(|c| c.is_alphabetic()).collect();
    if letters.len() >= 2 && letters.iter().all(|c| c.is_uppercase()) {
        return suggestion.to_uppercase();
    }
    let mut chars = raw.chars();
    if let Some(first) = chars.next() {
        if first.is_uppercase() && chars.all(|c| !c.is_alphabetic() || c.is_lowercase()) {
            let mut out = suggestion.chars();
            if let Some(head) = out.next() {
                return head.to_uppercase().collect::<String>() + out.as_str();
            }
        }
    }
    suggestion.to_string()
}

fn normalize_query(input: &str) -> Result<String, AppError> {
    let word = input.trim().to_lowercase();
    if word.is_empty() || word.chars().count() > 64 || !is_query_word(&word) {
        return Err(AppError::bad_request(
            "word must be 1-64 ASCII letters, ' or -",
        ));
    }
    Ok(word)
}

fn is_query_word(word: &str) -> bool {
    !word.is_empty()
        && word
            .chars()
            .all(|c| c.is_ascii_alphabetic() || c == '\'' || c == '-')
        && !word.starts_with('\'')
        && !word.ends_with('\'')
        && !word.starts_with('-')
        && !word.ends_with('-')
}

fn validate_text(state: &AppState, text: &str) -> Result<(), AppError> {
    if text.chars().count() > state.config.max_text_chars {
        return Err(AppError::bad_request(format!(
            "text exceeds the {} character limit",
            state.config.max_text_chars
        )));
    }
    Ok(())
}

fn round_ms(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}
