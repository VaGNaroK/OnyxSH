//! Zero-Trust AI Agent subsystem: Models, Secret Redactor, PathGuard, PolicyEngine,
//! SmartRouter, PlanParser, ContextManager, TerminalErrorMatcher, PostExecutionVerifier,
//! and AuditLogger.

pub mod audit;
pub mod client;
pub mod context_manager;
pub mod error_matcher;
pub mod models;
pub mod path_guard;
pub mod planner;
pub mod policy_engine;
pub mod redactor;
pub mod router;
pub mod verifier;
