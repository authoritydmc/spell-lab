use std::{path::Path, sync::Arc};

use anyhow::{Context, Result};
use chrono::{SecondsFormat, Utc};
use rand::{rngs::OsRng, RngCore};
use redb::{Database, ReadableTable, ReadableTableMetadata, TableDefinition};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const USERS: TableDefinition<&str, &[u8]> = TableDefinition::new("users");
const API_KEYS: TableDefinition<&str, &str> = TableDefinition::new("api_keys");
const USER_WORDS: TableDefinition<&str, ()> = TableDefinition::new("user_words");

const MAX_USER_WORDS: usize = 100_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredUser {
    pub id: String,
    pub name: String,
    pub api_key_hash: String,
    pub api_key_prefix: String,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct NewUser {
    pub user: StoredUser,
    pub api_key: String,
}

#[derive(Debug, Clone)]
pub struct WordPage {
    pub words: Vec<String>,
    pub total: usize,
}

#[derive(Clone)]
pub struct Db {
    db: Arc<Database>,
}

impl Db {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating database directory {}", parent.display()))?;
        }
        let db = Database::create(path)
            .with_context(|| format!("opening redb database {}", path.display()))?;
        let db = Self { db: Arc::new(db) };
        db.ensure_tables()?;
        Ok(db)
    }

    fn ensure_tables(&self) -> Result<()> {
        let txn = self.db.begin_write()?;
        {
            let _ = txn.open_table(USERS)?;
            let _ = txn.open_table(API_KEYS)?;
            let _ = txn.open_table(USER_WORDS)?;
        }
        txn.commit()?;
        Ok(())
    }

    pub fn create_user(&self, name: Option<String>) -> Result<NewUser> {
        let id = uuid::Uuid::new_v4().to_string();
        let api_key = generate_api_key();
        let hash = hash_api_key(&api_key);
        let prefix = key_prefix(&api_key);
        let name = name
            .map(|n| n.trim().to_string())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| "Anonymous".to_string());
        let name = truncate_chars(&name, 80);
        let user = StoredUser {
            id: id.clone(),
            name,
            api_key_hash: hash.clone(),
            api_key_prefix: prefix,
            created_at: now(),
        };

        let encoded = serde_json::to_vec(&user)?;
        let txn = self.db.begin_write()?;
        {
            let mut users = txn.open_table(USERS)?;
            let mut keys = txn.open_table(API_KEYS)?;
            users.insert(user.id.as_str(), encoded.as_slice())?;
            keys.insert(hash.as_str(), user.id.as_str())?;
        }
        txn.commit()?;

        Ok(NewUser { user, api_key })
    }

    pub fn user_by_api_key(&self, api_key: &str) -> Result<Option<StoredUser>> {
        let hash = hash_api_key(api_key);
        let txn = self.db.begin_read()?;
        let keys = txn.open_table(API_KEYS)?;
        let Some(user_id) = keys.get(hash.as_str())? else {
            return Ok(None);
        };
        let user_id = user_id.value().to_string();
        drop(keys);
        drop(txn);
        self.user_by_id(&user_id)
    }

    pub fn user_by_id(&self, user_id: &str) -> Result<Option<StoredUser>> {
        let txn = self.db.begin_read()?;
        let users = txn.open_table(USERS)?;
        let Some(raw) = users.get(user_id)? else {
            return Ok(None);
        };
        let user: StoredUser = serde_json::from_slice(raw.value())?;
        Ok(Some(user))
    }

    pub fn rotate_api_key(&self, user_id: &str) -> Result<Option<(StoredUser, String)>> {
        let Some(mut user) = self.user_by_id(user_id)? else {
            return Ok(None);
        };
        let old_hash = user.api_key_hash.clone();
        let api_key = generate_api_key();
        let hash = hash_api_key(&api_key);
        user.api_key_hash = hash.clone();
        user.api_key_prefix = key_prefix(&api_key);
        let encoded = serde_json::to_vec(&user)?;

        let txn = self.db.begin_write()?;
        {
            let mut users = txn.open_table(USERS)?;
            let mut keys = txn.open_table(API_KEYS)?;
            users.insert(user.id.as_str(), encoded.as_slice())?;
            keys.remove(old_hash.as_str())?;
            keys.insert(hash.as_str(), user.id.as_str())?;
        }
        txn.commit()?;
        Ok(Some((user, api_key)))
    }

    pub fn delete_user(&self, user_id: &str) -> Result<bool> {
        let Some(user) = self.user_by_id(user_id)? else {
            return Ok(false);
        };
        let keys = self.word_keys(user_id)?;

        let txn = self.db.begin_write()?;
        {
            let mut users = txn.open_table(USERS)?;
            let mut api_keys = txn.open_table(API_KEYS)?;
            let mut words = txn.open_table(USER_WORDS)?;
            users.remove(user.id.as_str())?;
            api_keys.remove(user.api_key_hash.as_str())?;
            for key in keys {
                words.remove(key.as_str())?;
            }
        }
        txn.commit()?;
        Ok(true)
    }

    fn word_keys(&self, user_id: &str) -> Result<Vec<String>> {
        let prefix = word_prefix(user_id);
        let txn = self.db.begin_read()?;
        let table = txn.open_table(USER_WORDS)?;
        let mut keys = Vec::new();
        for item in table.range(prefix.as_str()..)? {
            let (key, _) = item?;
            let key = key.value();
            if !key.starts_with(&prefix) {
                break;
            }
            keys.push(key.to_string());
        }
        Ok(keys)
    }

    pub fn all_user_words(&self, user_id: &str) -> Result<Vec<String>> {
        self.user_words(user_id, 0, usize::MAX).map(|p| p.words)
    }

    pub fn user_words(&self, user_id: &str, offset: usize, limit: usize) -> Result<WordPage> {
        let prefix = word_prefix(user_id);
        let txn = self.db.begin_read()?;
        let table = txn.open_table(USER_WORDS)?;
        let mut total = 0usize;
        let mut words = Vec::new();
        for item in table.range(prefix.as_str()..)? {
            let (key, _) = item?;
            let key = key.value();
            if !key.starts_with(&prefix) {
                break;
            }
            if total >= offset && words.len() < limit {
                words.push(key[prefix.len()..].to_string());
            }
            total += 1;
        }
        Ok(WordPage { words, total })
    }

    pub fn user_word_count(&self, user_id: &str) -> Result<usize> {
        Ok(self.user_words(user_id, 0, 0)?.total)
    }

    pub fn add_user_words(&self, user_id: &str, words: &[String]) -> Result<(usize, usize)> {
        let current = self.user_word_count(user_id)?;
        if current + words.len() > MAX_USER_WORDS {
            anyhow::bail!("user dictionary limit is {MAX_USER_WORDS} words");
        }

        let txn = self.db.begin_write()?;
        let mut added = 0usize;
        let mut duplicates = 0usize;
        {
            let mut table = txn.open_table(USER_WORDS)?;
            for word in words {
                let key = word_key(user_id, word);
                if table.get(key.as_str())?.is_some() {
                    duplicates += 1;
                } else {
                    table.insert(key.as_str(), ())?;
                    added += 1;
                }
            }
        }
        txn.commit()?;
        Ok((added, duplicates))
    }

    pub fn remove_user_word(&self, user_id: &str, word: &str) -> Result<bool> {
        let key = word_key(user_id, word);
        let txn = self.db.begin_write()?;
        let removed;
        {
            let mut table = txn.open_table(USER_WORDS)?;
            removed = table.remove(key.as_str())?.is_some();
        }
        txn.commit()?;
        Ok(removed)
    }

    pub fn clear_user_words(&self, user_id: &str) -> Result<usize> {
        let keys = self.word_keys(user_id)?;
        let txn = self.db.begin_write()?;
        {
            let mut table = txn.open_table(USER_WORDS)?;
            for key in &keys {
                table.remove(key.as_str())?;
            }
        }
        txn.commit()?;
        Ok(keys.len())
    }

    pub fn counts(&self) -> Result<(u64, u64)> {
        let txn = self.db.begin_read()?;
        let users = txn.open_table(USERS)?.len()?;
        let words = txn.open_table(USER_WORDS)?.len()?;
        Ok((users, words))
    }
}

pub fn normalize_word(input: &str) -> Option<String> {
    let word = input.trim().to_lowercase();
    if word.is_empty() || word.chars().count() > 64 {
        return None;
    }
    if !word
        .chars()
        .all(|c| c.is_ascii_alphabetic() || c == '\'' || c == '-')
    {
        return None;
    }
    if word.starts_with('-')
        || word.ends_with('-')
        || word.starts_with('\'')
        || word.ends_with('\'')
        || word.contains("--")
        || word.contains("''")
    {
        return None;
    }
    Some(word)
}

pub fn hash_api_key(api_key: &str) -> String {
    hex::encode(Sha256::digest(api_key.as_bytes()))
}

fn generate_api_key() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    format!("sl_{}", hex::encode(bytes))
}

fn key_prefix(api_key: &str) -> String {
    let visible = api_key.strip_prefix("sl_").unwrap_or(api_key);
    format!("sl_{}…{}", &visible[..6], &visible[visible.len() - 4..])
}

fn now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true)
}

fn truncate_chars(value: &str, max: usize) -> String {
    value.chars().take(max).collect()
}

fn word_prefix(user_id: &str) -> String {
    format!("{user_id}\u{1f}")
}

fn word_key(user_id: &str, word: &str) -> String {
    format!("{user_id}\u{1f}{word}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_words() {
        assert_eq!(normalize_word(" Hello ").as_deref(), Some("hello"));
        assert_eq!(normalize_word("don't").as_deref(), Some("don't"));
        assert_eq!(normalize_word("well-known").as_deref(), Some("well-known"));
        assert!(normalize_word("two words").is_none());
        assert!(normalize_word("word!").is_none());
        assert!(normalize_word("123").is_none());
        assert!(normalize_word("-word").is_none());
    }

    #[test]
    fn user_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(dir.path().join("test.db")).unwrap();
        let created = db.create_user(Some("Tester".into())).unwrap();
        let found = db.user_by_api_key(&created.api_key).unwrap().unwrap();
        assert_eq!(found.id, created.user.id);

        db.add_user_words(&created.user.id, &["kubernetes".into(), "grafana".into()])
            .unwrap();
        assert_eq!(db.user_word_count(&created.user.id).unwrap(), 2);
        let page = db.user_words(&created.user.id, 0, 10).unwrap();
        assert_eq!(page.words, vec!["grafana", "kubernetes"]);

        db.remove_user_word(&created.user.id, "grafana").unwrap();
        assert_eq!(db.user_word_count(&created.user.id).unwrap(), 1);

        let (_, new_key) = db.rotate_api_key(&created.user.id).unwrap().unwrap();
        assert!(db.user_by_api_key(&created.api_key).unwrap().is_none());
        assert!(db.user_by_api_key(&new_key).unwrap().is_some());

        assert!(db.delete_user(&created.user.id).unwrap());
        assert_eq!(db.counts().unwrap(), (0, 0));
    }
}
