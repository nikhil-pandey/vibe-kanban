//! Concurrency limiting service for task execution.
//!
//! This service enforces global and per-agent concurrency limits on coding agent executions.

use db::{
    DBService,
    models::execution_process::{ConcurrencyStats, ExecutionProcess},
};
use executors::executors::BaseCodingAgent;
use thiserror::Error;

use super::config::{ConcurrencyConfig, ConcurrencyLimit};

#[derive(Debug, Error)]
pub enum ConcurrencyError {
    #[error("Global concurrency limit reached: {current}/{limit} tasks running")]
    GlobalLimitReached { current: u32, limit: u32 },

    #[error("Concurrency limit reached for agent {agent}: {current}/{limit} tasks running")]
    AgentLimitReached {
        agent: String,
        current: u32,
        limit: u32,
    },

    #[error(transparent)]
    Database(#[from] sqlx::Error),
}

/// Result of checking concurrency limits
#[derive(Debug)]
pub enum ConcurrencyCheckResult {
    /// Execution is allowed to proceed
    Allowed,
    /// Global limit would be exceeded
    GlobalLimitExceeded { current: u32, limit: u32 },
    /// Agent-specific limit would be exceeded
    AgentLimitExceeded {
        agent: String,
        current: u32,
        limit: u32,
    },
}

impl ConcurrencyCheckResult {
    pub fn is_allowed(&self) -> bool {
        matches!(self, ConcurrencyCheckResult::Allowed)
    }

    pub fn into_result(self) -> Result<(), ConcurrencyError> {
        match self {
            ConcurrencyCheckResult::Allowed => Ok(()),
            ConcurrencyCheckResult::GlobalLimitExceeded { current, limit } => {
                Err(ConcurrencyError::GlobalLimitReached { current, limit })
            }
            ConcurrencyCheckResult::AgentLimitExceeded {
                agent,
                current,
                limit,
            } => Err(ConcurrencyError::AgentLimitReached {
                agent,
                current,
                limit,
            }),
        }
    }
}

/// Service for checking and enforcing concurrency limits
#[derive(Clone)]
pub struct ConcurrencyService {
    db: DBService,
}

impl ConcurrencyService {
    pub fn new(db: DBService) -> Self {
        Self { db }
    }

    /// Check if starting a new coding agent execution is allowed based on concurrency limits.
    ///
    /// This checks both global limits and agent-specific limits.
    pub async fn check_can_start_execution(
        &self,
        config: &ConcurrencyConfig,
        agent: &BaseCodingAgent,
    ) -> Result<ConcurrencyCheckResult, sqlx::Error> {
        let stats = ExecutionProcess::get_concurrency_stats(&self.db.pool).await?;

        // Check global limit first
        if let ConcurrencyLimit::Limited(limit) = config.global_limit {
            if stats.total_coding_agents >= limit {
                return Ok(ConcurrencyCheckResult::GlobalLimitExceeded {
                    current: stats.total_coding_agents,
                    limit,
                });
            }
        }

        // Check agent-specific limit
        let agent_name = agent.to_string();
        let effective_limit = config.effective_limit_for_agent(agent);

        if let ConcurrencyLimit::Limited(limit) = effective_limit {
            let current = stats.by_executor.get(&agent_name).copied().unwrap_or(0);
            if current >= *limit {
                return Ok(ConcurrencyCheckResult::AgentLimitExceeded {
                    agent: agent_name,
                    current,
                    limit: *limit,
                });
            }
        }

        Ok(ConcurrencyCheckResult::Allowed)
    }

    /// Get current concurrency statistics
    pub async fn get_stats(&self) -> Result<ConcurrencyStats, sqlx::Error> {
        ExecutionProcess::get_concurrency_stats(&self.db.pool).await
    }

    /// Check if a specific agent can start a new execution
    pub async fn can_start_for_agent(
        &self,
        config: &ConcurrencyConfig,
        agent: &BaseCodingAgent,
    ) -> Result<bool, sqlx::Error> {
        let result = self.check_can_start_execution(config, agent).await?;
        Ok(result.is_allowed())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn test_concurrency_check_result_is_allowed() {
        assert!(ConcurrencyCheckResult::Allowed.is_allowed());
        assert!(
            !ConcurrencyCheckResult::GlobalLimitExceeded {
                current: 5,
                limit: 5
            }
            .is_allowed()
        );
        assert!(
            !ConcurrencyCheckResult::AgentLimitExceeded {
                agent: "ClaudeCode".to_string(),
                current: 2,
                limit: 2
            }
            .is_allowed()
        );
    }

    #[test]
    fn test_concurrency_check_into_result() {
        assert!(ConcurrencyCheckResult::Allowed.into_result().is_ok());

        let result = ConcurrencyCheckResult::GlobalLimitExceeded {
            current: 5,
            limit: 5,
        }
        .into_result();
        assert!(matches!(
            result,
            Err(ConcurrencyError::GlobalLimitReached { .. })
        ));

        let result = ConcurrencyCheckResult::AgentLimitExceeded {
            agent: "ClaudeCode".to_string(),
            current: 2,
            limit: 2,
        }
        .into_result();
        assert!(matches!(
            result,
            Err(ConcurrencyError::AgentLimitReached { .. })
        ));
    }
}
