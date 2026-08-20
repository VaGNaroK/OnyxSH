"""Agent package for OnyxSH Secure Agent Mode."""

from .admin_tools import AdminTools
from .audit import AuditLogger
from .context_manager import ContextManager
from .fs_tools import FSTools
from .models import (
    ActionPlan,
    ActionStep,
    AuditRecord,
    RiskLevel,
    ToolResult,
)
from .orchestrator import AgentOrchestrator
from .path_guard import PathGuard
from .planner import PlanParser
from .policy_engine import PolicyEngine
from .providers import LLMProvider, get_provider
from .redactor import redact_secrets
from .shell_tools import run_argv
from .tool_registry import ExecContext, ToolRegistry

from .verifier import PostVerifier, VerificationCheck, VerificationResult
from .router import (
    SmartRouter,
    RoutingProfile,
    TaskComplexity,
    RouteDecision,
    TaskComplexityClassifier,
)

__all__ = [
    "RiskLevel",
    "ActionStep",
    "ActionPlan",
    "ToolResult",
    "AuditRecord",
    "AuditLogger",
    "PathGuard",
    "PolicyEngine",
    "ToolRegistry",
    "ExecContext",
    "FSTools",
    "AdminTools",
    "ContextManager",
    "PlanParser",
    "AgentOrchestrator",
    "LLMProvider",
    "get_provider",
    "redact_secrets",
    "run_argv",
    "PostVerifier",
    "VerificationCheck",
    "VerificationResult",
    "SmartRouter",
    "RoutingProfile",
    "TaskComplexity",
    "RouteDecision",
    "TaskComplexityClassifier",
]
