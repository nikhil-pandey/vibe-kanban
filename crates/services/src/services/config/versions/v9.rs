use std::collections::HashMap;

use anyhow::Error;
use executors::{executors::BaseCodingAgent, profile::ExecutorProfileId};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
pub use v8::{
    EditorConfig, EditorType, GitHubConfig, NotificationConfig, ShowcaseState, SoundFile,
    ThemeMode, UiLanguage,
};

use crate::services::config::versions::v8;

fn default_git_branch_prefix() -> String {
    "vk".to_string()
}

fn default_pr_auto_description_enabled() -> bool {
    true
}

/// Concurrency limit value - either a specific number or unlimited
#[derive(Clone, Debug, Serialize, Deserialize, TS, PartialEq)]
#[serde(untagged)]
pub enum ConcurrencyLimit {
    /// A specific numeric limit (must be >= 1)
    Limited(u32),
    /// Unlimited concurrency (represented as null in JSON)
    #[serde(serialize_with = "serialize_unlimited", deserialize_with = "deserialize_unlimited")]
    Unlimited,
}

fn serialize_unlimited<S>(serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_none()
}

fn deserialize_unlimited<'de, D>(_deserializer: D) -> Result<(), D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(())
}

impl Default for ConcurrencyLimit {
    fn default() -> Self {
        ConcurrencyLimit::Unlimited
    }
}

impl ConcurrencyLimit {
    pub fn is_unlimited(&self) -> bool {
        matches!(self, ConcurrencyLimit::Unlimited)
    }

    pub fn limit(&self) -> Option<u32> {
        match self {
            ConcurrencyLimit::Limited(n) => Some(*n),
            ConcurrencyLimit::Unlimited => None,
        }
    }

    pub fn allows(&self, current_count: u32) -> bool {
        match self {
            ConcurrencyLimit::Unlimited => true,
            ConcurrencyLimit::Limited(max) => current_count < *max,
        }
    }
}

/// Queue behavior configuration
#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct QueueConfig {
    /// Whether to auto-queue tasks when concurrency limit is reached (vs reject with 429)
    #[serde(default = "default_queue_enabled")]
    pub enabled: bool,

    /// Whether to auto-resume interrupted tasks on server restart
    #[serde(default = "default_resume_on_restart")]
    pub resume_on_restart: bool,

    /// Custom prompt prefix to add when resuming an interrupted task
    /// Use {original_prompt} as placeholder for the original prompt
    #[serde(default = "default_resume_prompt")]
    pub resume_prompt: String,
}

fn default_queue_enabled() -> bool {
    true
}

fn default_resume_on_restart() -> bool {
    true
}

fn default_resume_prompt() -> String {
    "[Process restarted. Continue]".to_string()
}

impl Default for QueueConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            resume_on_restart: true,
            resume_prompt: default_resume_prompt(),
        }
    }
}

/// Concurrency settings for task execution
#[derive(Clone, Debug, Serialize, Deserialize, TS, Default)]
pub struct ConcurrencyConfig {
    /// Global maximum concurrent tasks across all agents (null = unlimited)
    #[serde(default)]
    pub global_limit: ConcurrencyLimit,

    /// Per-agent concurrency limits (agent name -> limit, null = unlimited)
    /// If an agent is not in this map, it uses the global limit
    #[serde(default)]
    #[ts(type = "Record<string, number | null>")]
    pub agent_limits: HashMap<String, ConcurrencyLimit>,

    /// Queue behavior configuration
    #[serde(default)]
    pub queue: QueueConfig,
}

impl ConcurrencyConfig {
    /// Get the effective limit for a specific agent
    pub fn effective_limit_for_agent(&self, agent: &BaseCodingAgent) -> &ConcurrencyLimit {
        let agent_name = agent.to_string();
        self.agent_limits.get(&agent_name).unwrap_or(&self.global_limit)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, TS)]
pub struct Config {
    pub config_version: String,
    pub theme: ThemeMode,
    pub executor_profile: ExecutorProfileId,
    pub disclaimer_acknowledged: bool,
    pub onboarding_acknowledged: bool,
    pub notifications: NotificationConfig,
    pub editor: EditorConfig,
    pub github: GitHubConfig,
    pub analytics_enabled: bool,
    pub workspace_dir: Option<String>,
    pub last_app_version: Option<String>,
    pub show_release_notes: bool,
    #[serde(default)]
    pub language: UiLanguage,
    #[serde(default = "default_git_branch_prefix")]
    pub git_branch_prefix: String,
    #[serde(default)]
    pub showcases: ShowcaseState,
    #[serde(default = "default_pr_auto_description_enabled")]
    pub pr_auto_description_enabled: bool,
    #[serde(default)]
    pub pr_auto_description_prompt: Option<String>,
    /// Concurrency settings for task execution
    #[serde(default)]
    pub concurrency: ConcurrencyConfig,
}

impl Config {
    fn from_v8_config(old_config: v8::Config) -> Self {
        Self {
            config_version: "v9".to_string(),
            theme: old_config.theme,
            executor_profile: old_config.executor_profile,
            disclaimer_acknowledged: old_config.disclaimer_acknowledged,
            onboarding_acknowledged: old_config.onboarding_acknowledged,
            notifications: old_config.notifications,
            editor: old_config.editor,
            github: old_config.github,
            analytics_enabled: old_config.analytics_enabled,
            workspace_dir: old_config.workspace_dir,
            last_app_version: old_config.last_app_version,
            show_release_notes: old_config.show_release_notes,
            language: old_config.language,
            git_branch_prefix: old_config.git_branch_prefix,
            showcases: old_config.showcases,
            pr_auto_description_enabled: old_config.pr_auto_description_enabled,
            pr_auto_description_prompt: old_config.pr_auto_description_prompt,
            concurrency: ConcurrencyConfig::default(),
        }
    }

    pub fn from_previous_version(raw_config: &str) -> Result<Self, Error> {
        let old_config = v8::Config::from(raw_config.to_string());
        Ok(Self::from_v8_config(old_config))
    }
}

impl From<String> for Config {
    fn from(raw_config: String) -> Self {
        if let Ok(config) = serde_json::from_str::<Config>(&raw_config)
            && config.config_version == "v9"
        {
            return config;
        }

        match Self::from_previous_version(&raw_config) {
            Ok(config) => {
                tracing::info!("Config upgraded to v9");
                config
            }
            Err(e) => {
                tracing::warn!("Config migration failed: {}, using default", e);
                Self::default()
            }
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            config_version: "v9".to_string(),
            theme: ThemeMode::System,
            executor_profile: ExecutorProfileId::new(BaseCodingAgent::ClaudeCode),
            disclaimer_acknowledged: false,
            onboarding_acknowledged: false,
            notifications: NotificationConfig::default(),
            editor: EditorConfig::default(),
            github: GitHubConfig::default(),
            analytics_enabled: true,
            workspace_dir: None,
            last_app_version: None,
            show_release_notes: false,
            language: UiLanguage::default(),
            git_branch_prefix: default_git_branch_prefix(),
            showcases: ShowcaseState::default(),
            pr_auto_description_enabled: true,
            pr_auto_description_prompt: None,
            concurrency: ConcurrencyConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_concurrency_limit_allows() {
        let unlimited = ConcurrencyLimit::Unlimited;
        assert!(unlimited.allows(0));
        assert!(unlimited.allows(100));
        assert!(unlimited.allows(u32::MAX));

        let limited = ConcurrencyLimit::Limited(3);
        assert!(limited.allows(0));
        assert!(limited.allows(1));
        assert!(limited.allows(2));
        assert!(!limited.allows(3));
        assert!(!limited.allows(4));
    }

    #[test]
    fn test_concurrency_limit_serialization() {
        // Test Limited serialization
        let limited = ConcurrencyLimit::Limited(5);
        let json = serde_json::to_string(&limited).unwrap();
        assert_eq!(json, "5");

        // Test deserialization of number
        let deserialized: ConcurrencyLimit = serde_json::from_str("5").unwrap();
        assert_eq!(deserialized, ConcurrencyLimit::Limited(5));

        // Test deserialization of null as Unlimited
        let deserialized: ConcurrencyLimit = serde_json::from_str("null").unwrap();
        assert_eq!(deserialized, ConcurrencyLimit::Unlimited);
    }

    #[test]
    fn test_concurrency_config_effective_limit() {
        let mut config = ConcurrencyConfig {
            global_limit: ConcurrencyLimit::Limited(5),
            agent_limits: HashMap::new(),
            queue: QueueConfig::default(),
        };

        // Without agent-specific limit, use global
        let effective = config.effective_limit_for_agent(&BaseCodingAgent::ClaudeCode);
        assert_eq!(effective, &ConcurrencyLimit::Limited(5));

        // With agent-specific limit, use that
        config.agent_limits.insert(
            BaseCodingAgent::ClaudeCode.to_string(),
            ConcurrencyLimit::Limited(2),
        );
        let effective = config.effective_limit_for_agent(&BaseCodingAgent::ClaudeCode);
        assert_eq!(effective, &ConcurrencyLimit::Limited(2));

        // Other agents still use global
        let effective = config.effective_limit_for_agent(&BaseCodingAgent::Cursor);
        assert_eq!(effective, &ConcurrencyLimit::Limited(5));
    }
}
