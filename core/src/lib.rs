//! Encrypted, in-memory-only note store.
//!
//! Notes exist only for the lifetime of the process. Content is sealed with
//! ChaCha20-Poly1305 under a random session key that is never written to disk,
//! and plaintext buffers are zeroized after use.

use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Key, Nonce,
};
use rand::rngs::SysRng;
use rand::TryRng;
use std::collections::HashMap;
use std::fmt;
use zeroize::{Zeroize, Zeroizing};

/// Maximum note size in bytes (1 MiB). Bounds memory use under `mlockall`.
pub const MAX_NOTE_BYTES: usize = 1 << 20;

pub struct NoteStore {
    cipher: ChaCha20Poly1305,
    sealed: HashMap<String, SealedNote>,
}

struct SealedNote {
    ct: Vec<u8>,
    created: u64,
}

impl Drop for SealedNote {
    fn drop(&mut self) {
        self.ct.zeroize();
    }
}

impl NoteStore {
    /// Fallible constructor: an RNG failure is reported as an error, never a
    /// panic, so it is safe to call from inside a GTK callback (a panic there
    /// would abort across the FFI boundary).
    pub fn try_new() -> Result<Self, StoreError> {
        let mut key_bytes = [0u8; 32];
        SysRng
            .try_fill_bytes(&mut key_bytes)
            .map_err(|_| StoreError::Seal)?;
        let key = Key::from(key_bytes);
        key_bytes.zeroize();
        Ok(Self {
            cipher: ChaCha20Poly1305::new(&key),
            sealed: HashMap::new(),
        })
    }

    pub fn new() -> Self {
        Self::try_new().expect("system RNG failure")
    }

    fn seal(&self, plaintext: &str) -> Result<SealedNote, StoreError> {
        let mut nonce_bytes = [0u8; 12];
        SysRng
            .try_fill_bytes(&mut nonce_bytes)
            .map_err(|_| StoreError::Seal)?;
        let nonce = Nonce::from(nonce_bytes);
        let mut ct = self
            .cipher
            .encrypt(&nonce, plaintext.as_bytes())
            .map_err(|_| StoreError::Seal)?;
        let mut sealed_ct = nonce_bytes.to_vec();
        sealed_ct.extend_from_slice(&ct);
        ct.zeroize();
        Ok(SealedNote {
            ct: sealed_ct,
            created: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs()),
        })
    }

    fn open(&self, sealed: &SealedNote) -> Result<Zeroizing<String>, StoreError> {
        if sealed.ct.len() < 12 {
            return Err(StoreError::Corrupt);
        }
        let (nonce_bytes, body) = sealed.ct.split_at(12);
        let nonce = Nonce::try_from(nonce_bytes).map_err(|_| StoreError::Corrupt)?;
        let pt = self
            .cipher
            .decrypt(&nonce, body)
            .map_err(|_| StoreError::Corrupt)?;
        let pt = Zeroizing::new(pt);
        match String::from_utf8(pt.to_vec()) {
            Ok(s) => Ok(Zeroizing::new(s)),
            Err(e) => {
                let mut bytes = e.into_bytes();
                bytes.zeroize();
                Err(StoreError::Corrupt)
            }
        }
    }

    /// Create or overwrite a note. The caller is responsible for zeroizing
    /// its own copy of the plaintext; this function only seals what it is given.
    pub fn put(&mut self, id: &str, content: &str) -> Result<(), StoreError> {
        if content.len() > MAX_NOTE_BYTES {
            return Err(StoreError::TooLarge);
        }
        let sealed = self.seal(content)?;
        self.sealed.insert(id.to_string(), sealed);
        Ok(())
    }

    /// Read a note. The returned buffer zeroizes itself when dropped.
    pub fn get(&self, id: &str) -> Result<Zeroizing<String>, StoreError> {
        self.sealed
            .get(id)
            .ok_or(StoreError::NotFound)
            .and_then(|s| self.open(s))
    }

    pub fn delete(&mut self, id: &str) -> bool {
        self.sealed.remove(id).is_some()
    }

    pub fn ids(&self) -> Vec<String> {
        let mut v: Vec<String> = self.sealed.keys().cloned().collect();
        v.sort();
        v
    }

    pub fn len(&self) -> usize {
        self.sealed.len()
    }

    /// Note creation timestamp (seconds since UNIX epoch), if the id exists.
    pub fn created_at(&self, id: &str) -> Option<u64> {
        self.sealed.get(id).map(|s| s.created)
    }

    pub fn is_empty(&self) -> bool {
        self.sealed.is_empty()
    }

    pub fn search(&self, query: &str) -> Vec<String> {
        let needle = Zeroizing::new(query.to_lowercase());
        if needle.is_empty() {
            return self.ids();
        }
        self.ids()
            .into_iter()
            .filter(|id| self.matches(id, &needle))
            .collect()
    }

    fn matches(&self, id: &str, needle: &str) -> bool {
        self.get(id)
            .map(|content| {
                let lower = Zeroizing::new(content.to_lowercase());
                lower.contains(needle)
            })
            .unwrap_or(false)
    }

    pub fn clear(&mut self) {
        self.sealed.clear();
    }
}

impl Default for NoteStore {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreError {
    NotFound,
    Seal,
    Corrupt,
    TooLarge,
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => write!(f, "note not found"),
            Self::Seal => write!(f, "failed to seal note"),
            Self::Corrupt => write!(f, "note is corrupt or tampered"),
            Self::TooLarge => write!(f, "note exceeds the maximum size"),
        }
    }
}

impl std::error::Error for StoreError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn put_get_roundtrip() {
        let mut store = NoteStore::new();
        store.put("a", "mot de passe temporaire").unwrap();
        let v = store.get("a").unwrap();
        assert_eq!(v.as_str(), "mot de passe temporaire");
    }

    #[test]
    fn missing_note() {
        let store = NoteStore::new();
        assert_eq!(store.get("nope"), Err(StoreError::NotFound));
    }

    #[test]
    fn overwrite_and_delete() {
        let mut store = NoteStore::new();
        store.put("a", "one").unwrap();
        store.put("a", "two").unwrap();
        assert_eq!(store.get("a").unwrap().as_str(), "two");
        assert!(store.delete("a"));
        assert!(!store.delete("a"));
        assert!(store.is_empty());
    }

    #[test]
    fn tampered_ciphertext_fails() {
        let mut store = NoteStore::new();
        store.put("a", "secret").unwrap();
        // Reach into the map and flip a byte of the ciphertext body.
        let sealed = store.sealed.get_mut("a").unwrap();
        let last = sealed.ct.len() - 1;
        sealed.ct[last] ^= 0x01;
        assert_eq!(store.get("a"), Err(StoreError::Corrupt));
    }

    #[test]
    fn ids_sorted() {
        let mut store = NoteStore::new();
        store.put("b", "x").unwrap();
        store.put("a", "y").unwrap();
        assert_eq!(store.ids(), vec!["a".to_string(), "b".to_string()]);
    }

    #[test]
    fn search_matches_case_insensitive() {
        let mut store = NoteStore::new();
        store.put("a", "Mot de Passe bancaire").unwrap();
        store.put("b", "liste de courses").unwrap();
        assert_eq!(store.search("mot de passe"), vec!["a".to_string()]);
    }

    #[test]
    fn empty_query_returns_all_ids() {
        let mut store = NoteStore::new();
        store.put("a", "x").unwrap();
        store.put("b", "y").unwrap();
        assert_eq!(store.search("").len(), 2);
    }

    #[test]
    fn try_new_succeeds() {
        assert!(NoteStore::try_new().is_ok());
    }

    #[test]
    fn oversized_note_rejected() {
        let mut store = NoteStore::new();
        let big = "x".repeat(MAX_NOTE_BYTES + 1);
        assert_eq!(store.put("a", &big), Err(StoreError::TooLarge));
        assert!(store.is_empty());
    }

    #[test]
    fn clear_removes_everything() {
        let mut store = NoteStore::new();
        store.put("a", "secret").unwrap();
        store.clear();
        assert!(store.is_empty());
        assert_eq!(store.get("a"), Err(StoreError::NotFound));
    }
}
