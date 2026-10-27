//! Browser localStorage settings store implementation for WebAssembly.

use keypeek_presentation::{MemorySettingsStore, Settings, SettingsStore};
use std::sync::Arc;

pub const DEFAULT_SETTINGS_STORAGE_KEY: &str = "keypeek_settings";

/// Settings store backed by browser `localStorage`, with an in-memory fallback.
#[derive(Debug, Clone)]
pub struct LocalStorageSettingsStore {
    key: String,
    fallback: Arc<MemorySettingsStore>,
}

impl LocalStorageSettingsStore {
    /// Creates a new `LocalStorageSettingsStore` with a custom localStorage key.
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            fallback: Arc::new(MemorySettingsStore::default()),
        }
    }

    /// Access the underlying browser `localStorage` instance if available.
    fn storage() -> Option<web_sys::Storage> {
        let window = web_sys::window()?;
        window.local_storage().ok()?
    }

    /// Clears stored settings from browser `localStorage` and resets the in-memory fallback.
    pub fn clear(&self) -> Result<(), String> {
        let _ = self.fallback.save(&Settings::default());
        if let Some(storage) = Self::storage() {
            storage
                .remove_item(&self.key)
                .map_err(|e| format!("Failed to remove settings from localStorage: {e:?}"))?;
        }
        Ok(())
    }
}

impl Default for LocalStorageSettingsStore {
    fn default() -> Self {
        Self::new(DEFAULT_SETTINGS_STORAGE_KEY)
    }
}

impl SettingsStore for LocalStorageSettingsStore {
    fn load(&self) -> Settings {
        if let Some(storage) = Self::storage() {
            if let Ok(Some(ini_str)) = storage.get_item(&self.key) {
                if let Some(settings) = Settings::from_ini_str(&ini_str) {
                    let _ = self.fallback.save(&settings);
                    return settings;
                } else {
                    log::warn!(
                        "Failed to parse stored settings from localStorage key '{}'; falling back to defaults",
                        self.key
                    );
                }
            }
        }
        self.fallback.load()
    }

    fn save(&self, settings: &Settings) -> Result<(), String> {
        let _ = self.fallback.save(settings);
        let ini_text = settings.to_ini_string();

        if let Some(storage) = Self::storage() {
            storage
                .set_item(&self.key, &ini_text)
                .map_err(|e| format!("Failed to save settings to localStorage: {e:?}"))?;
        } else {
            log::warn!("localStorage is not available; settings retained in memory only");
        }

        Ok(())
    }
}
