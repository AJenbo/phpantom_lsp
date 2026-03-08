//! Per-project configuration loaded from `.phpantom.toml`.
//!
//! The config file lives in the project root (next to `composer.json`)
//! and controls optional features like diagnostic toggles and PHP
//! version overrides.  When the file is missing, all settings use their
//! defaults.
//!
//! See `docs/todo/config.md` for the full specification of planned
//! settings.  Only settings that are actually wired up appear here.

use std::path::Path;

use serde::Deserialize;

/// Top-level configuration parsed from `.phpantom.toml`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    /// PHP version and language settings.
    pub php: PhpConfig,
    /// Diagnostic toggles.
    pub diagnostics: DiagnosticsConfig,
    /// Indexing strategy and file discovery settings.
    pub indexing: IndexingConfig,
}

/// `[php]` section — PHP version override.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct PhpConfig {
    /// Override the detected PHP version (e.g. `"8.3"`).
    /// When `None`, PHPantom infers from `composer.json`.
    pub version: Option<String>,
}

/// `[diagnostics]` section — toggle individual diagnostic providers.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct DiagnosticsConfig {
    /// Report member access on subjects whose type could not be resolved.
    ///
    /// Off by default. When enabled, PHPantom emits a hint-level
    /// diagnostic on every `->`, `?->`, or `::` access where the
    /// subject type is unknown (e.g. `mixed`, untyped variable, or a
    /// return type PHPantom cannot infer). This is useful for
    /// discovering gaps in type coverage but produces too many
    /// diagnostics on codebases without comprehensive type annotations.
    #[serde(rename = "unresolved-member-access")]
    pub unresolved_member_access: Option<bool>,
}

impl DiagnosticsConfig {
    /// Whether the unresolved-member-access diagnostic is enabled.
    ///
    /// Defaults to `false` (off) when not explicitly set.
    pub fn unresolved_member_access_enabled(&self) -> bool {
        self.unresolved_member_access.unwrap_or(false)
    }
}

/// `[indexing]` section — controls how PHPantom discovers classes across
/// the workspace.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct IndexingConfig {
    /// The indexing strategy.
    ///
    /// - `"composer"` (default) — use Composer's classmap when available,
    ///   fall back to self-scan when it is missing or incomplete.
    /// - `"self"` — always build the classmap ourselves, ignoring
    ///   Composer's generated classmap entirely.
    /// - `"full"` — background-parse every PHP file for rich intelligence
    ///   (not yet implemented, treated as `"self"` for now).
    /// - `"none"` — no proactive scanning. Still uses Composer's classmap
    ///   if present, still resolves on demand, but never falls back to
    ///   self-scan.
    pub strategy: IndexingStrategy,
}

impl Default for IndexingConfig {
    fn default() -> Self {
        Self {
            strategy: IndexingStrategy::Composer,
        }
    }
}

/// The indexing strategy that controls class discovery behaviour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IndexingStrategy {
    /// Use Composer's classmap when available and complete. Fall back to
    /// self-scan when the classmap is missing or incomplete.
    #[default]
    Composer,
    /// Always build the classmap ourselves, ignoring Composer's generated
    /// classmap.
    SelfScan,
    /// Background-parse every PHP file for rich intelligence.
    Full,
    /// No proactive scanning. Uses Composer's classmap if present but
    /// never falls back to self-scan.
    None,
}

impl<'de> Deserialize<'de> for IndexingStrategy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "composer" => Ok(IndexingStrategy::Composer),
            "self" => Ok(IndexingStrategy::SelfScan),
            "full" => Ok(IndexingStrategy::Full),
            "none" => Ok(IndexingStrategy::None),
            other => Err(serde::de::Error::unknown_variant(
                other,
                &["composer", "self", "full", "none"],
            )),
        }
    }
}

impl std::fmt::Display for IndexingStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IndexingStrategy::Composer => write!(f, "composer"),
            IndexingStrategy::SelfScan => write!(f, "self"),
            IndexingStrategy::Full => write!(f, "full"),
            IndexingStrategy::None => write!(f, "none"),
        }
    }
}

/// The config file name that PHPantom looks for in the project root.
pub const CONFIG_FILE_NAME: &str = ".phpantom.toml";

/// Default content for a newly created `.phpantom.toml` file.
///
/// All settings are commented out so that the file is a self-documenting
/// reference. The user uncomments the lines they want to change.
pub const DEFAULT_CONFIG_CONTENT: &str = r#"# PHPantom project configuration
# https://github.com/AJenbo/phpantom_lsp

[php]
# Override the detected PHP version.
# When unset, PHPantom infers from composer.json's platform or require.php.
# version = "8.5"

[diagnostics]
# Report member access on subjects whose type could not be resolved.
# Useful for discovering gaps in type coverage. Off by default.
# unresolved-member-access = true

[indexing]
# How PHPantom discovers classes across the workspace.
#   "composer" (default) - use Composer classmap, self-scan on fallback
#   "self"    - always self-scan, ignore Composer classmap
#   "full"    - background-parse all project files (not yet implemented)
#   "none"    - no proactive scanning, Composer classmap only
# strategy = "composer"
"#;

/// Create a default `.phpantom.toml` in the given workspace root.
///
/// Returns `Ok(true)` if the file was created, `Ok(false)` if it
/// already exists, or `Err` on I/O failure.
pub fn create_default_config(workspace_root: &Path) -> Result<bool, ConfigError> {
    let config_path = workspace_root.join(CONFIG_FILE_NAME);

    if config_path.exists() {
        return Ok(false);
    }

    std::fs::write(&config_path, DEFAULT_CONFIG_CONTENT).map_err(|e| ConfigError::Io {
        path: config_path.display().to_string(),
        source: e,
    })?;

    Ok(true)
}

/// Load the project configuration from `.phpantom.toml` in the given
/// workspace root directory.
///
/// Returns `Config::default()` when the file does not exist or cannot
/// be parsed.  Parse errors are returned as `Err` so the caller can
/// log a warning to the user.
pub fn load_config(workspace_root: &Path) -> Result<Config, ConfigError> {
    let config_path = workspace_root.join(CONFIG_FILE_NAME);

    if !config_path.exists() {
        return Ok(Config::default());
    }

    let content = std::fs::read_to_string(&config_path).map_err(|e| ConfigError::Io {
        path: config_path.display().to_string(),
        source: e,
    })?;

    let config: Config = toml::from_str(&content).map_err(|e| ConfigError::Parse {
        path: config_path.display().to_string(),
        source: e,
    })?;

    Ok(config)
}

/// Errors that can occur when loading the config file.
#[derive(Debug)]
pub enum ConfigError {
    /// Failed to read the config file from disk.
    Io {
        /// Path that was attempted.
        path: String,
        /// The underlying I/O error.
        source: std::io::Error,
    },
    /// The config file contains invalid TOML or does not match the schema.
    Parse {
        /// Path that was attempted.
        path: String,
        /// The underlying TOML parse error.
        source: toml::de::Error,
    },
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::Io { path, source } => {
                write!(f, "failed to read {}: {}", path, source)
            }
            ConfigError::Parse { path, source } => {
                write!(f, "failed to parse {}: {}", path, source)
            }
        }
    }
}

impl std::error::Error for ConfigError {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn create_default_writes_file() {
        let dir = tempfile::tempdir().unwrap();
        let result = create_default_config(dir.path()).unwrap();
        assert!(result, "should report that the file was created");
        let path = dir.path().join(CONFIG_FILE_NAME);
        assert!(path.exists());
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("[php]"));
        assert!(content.contains("[diagnostics]"));
        assert!(content.contains("unresolved-member-access"));
    }

    #[test]
    fn create_default_does_not_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(CONFIG_FILE_NAME);
        std::fs::write(&path, "# custom\n").unwrap();
        let result = create_default_config(dir.path()).unwrap();
        assert!(!result, "should report that the file already exists");
        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            content, "# custom\n",
            "existing file must not be overwritten"
        );
    }

    #[test]
    fn default_content_parses_successfully() {
        let config: Config = toml::from_str(DEFAULT_CONFIG_CONTENT).unwrap();
        assert!(config.php.version.is_none());
        assert!(!config.diagnostics.unresolved_member_access_enabled());
        assert_eq!(config.indexing.strategy, IndexingStrategy::Composer);
    }

    #[test]
    fn missing_file_returns_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let config = load_config(dir.path()).unwrap();
        assert!(config.php.version.is_none());
        assert!(!config.diagnostics.unresolved_member_access_enabled());
        assert_eq!(config.indexing.strategy, IndexingStrategy::Composer);
    }

    #[test]
    fn empty_file_returns_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(CONFIG_FILE_NAME);
        std::fs::write(&path, "").unwrap();
        let config = load_config(dir.path()).unwrap();
        assert!(config.php.version.is_none());
        assert!(!config.diagnostics.unresolved_member_access_enabled());
        assert_eq!(config.indexing.strategy, IndexingStrategy::Composer);
    }

    #[test]
    fn parses_php_version() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(CONFIG_FILE_NAME);
        std::fs::write(&path, "[php]\nversion = \"8.3\"\n").unwrap();
        let config = load_config(dir.path()).unwrap();
        assert_eq!(config.php.version.as_deref(), Some("8.3"));
    }

    #[test]
    fn parses_diagnostics_section() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(CONFIG_FILE_NAME);
        std::fs::write(&path, "[diagnostics]\nunresolved-member-access = true\n").unwrap();
        let config = load_config(dir.path()).unwrap();
        assert!(config.diagnostics.unresolved_member_access_enabled());
    }

    #[test]
    fn unresolved_member_access_defaults_to_false() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(CONFIG_FILE_NAME);
        std::fs::write(&path, "[diagnostics]\n").unwrap();
        let config = load_config(dir.path()).unwrap();
        assert!(!config.diagnostics.unresolved_member_access_enabled());
    }

    #[test]
    fn invalid_toml_returns_parse_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(CONFIG_FILE_NAME);
        std::fs::write(&path, "[diagnostics\nbroken").unwrap();
        let result = load_config(dir.path());
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("failed to parse"));
    }

    #[test]
    fn unknown_keys_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(CONFIG_FILE_NAME);
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, "[diagnostics]").unwrap();
        writeln!(f, "unresolved-member-access = true").unwrap();
        writeln!(f, "some-future-tool = false").unwrap();
        drop(f);
        // Unknown keys should NOT cause a parse error — forward compatibility.
        let config = load_config(dir.path()).unwrap();
        assert!(config.diagnostics.unresolved_member_access_enabled());
    }

    #[test]
    fn unknown_sections_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(CONFIG_FILE_NAME);
        std::fs::write(
            &path,
            "[php]\nversion = \"8.4\"\n\n[some-future-section]\nkey = \"value\"\n",
        )
        .unwrap();
        let config = load_config(dir.path()).unwrap();
        assert_eq!(config.php.version.as_deref(), Some("8.4"));
    }

    #[test]
    fn full_example_config() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(CONFIG_FILE_NAME);
        std::fs::write(
            &path,
            r#"
[php]
version = "8.2"

[diagnostics]
unresolved-member-access = true

[indexing]
strategy = "self"
"#,
        )
        .unwrap();
        let config = load_config(dir.path()).unwrap();
        assert_eq!(config.php.version.as_deref(), Some("8.2"));
        assert!(config.diagnostics.unresolved_member_access_enabled());
        assert_eq!(config.indexing.strategy, IndexingStrategy::SelfScan);
    }

    #[test]
    fn parses_indexing_strategy_composer() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(CONFIG_FILE_NAME);
        std::fs::write(&path, "[indexing]\nstrategy = \"composer\"\n").unwrap();
        let config = load_config(dir.path()).unwrap();
        assert_eq!(config.indexing.strategy, IndexingStrategy::Composer);
    }

    #[test]
    fn parses_indexing_strategy_self() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(CONFIG_FILE_NAME);
        std::fs::write(&path, "[indexing]\nstrategy = \"self\"\n").unwrap();
        let config = load_config(dir.path()).unwrap();
        assert_eq!(config.indexing.strategy, IndexingStrategy::SelfScan);
    }

    #[test]
    fn parses_indexing_strategy_full() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(CONFIG_FILE_NAME);
        std::fs::write(&path, "[indexing]\nstrategy = \"full\"\n").unwrap();
        let config = load_config(dir.path()).unwrap();
        assert_eq!(config.indexing.strategy, IndexingStrategy::Full);
    }

    #[test]
    fn parses_indexing_strategy_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(CONFIG_FILE_NAME);
        std::fs::write(&path, "[indexing]\nstrategy = \"none\"\n").unwrap();
        let config = load_config(dir.path()).unwrap();
        assert_eq!(config.indexing.strategy, IndexingStrategy::None);
    }

    #[test]
    fn invalid_indexing_strategy_returns_parse_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(CONFIG_FILE_NAME);
        std::fs::write(&path, "[indexing]\nstrategy = \"bogus\"\n").unwrap();
        let result = load_config(dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn indexing_strategy_defaults_to_composer() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(CONFIG_FILE_NAME);
        std::fs::write(&path, "[indexing]\n").unwrap();
        let config = load_config(dir.path()).unwrap();
        assert_eq!(config.indexing.strategy, IndexingStrategy::Composer);
    }

    #[test]
    fn indexing_strategy_display() {
        assert_eq!(IndexingStrategy::Composer.to_string(), "composer");
        assert_eq!(IndexingStrategy::SelfScan.to_string(), "self");
        assert_eq!(IndexingStrategy::Full.to_string(), "full");
        assert_eq!(IndexingStrategy::None.to_string(), "none");
    }
}
