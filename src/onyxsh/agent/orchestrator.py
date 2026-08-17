"""Central orchestrator for the OnyxSH Secure Agent."""

from __future__ import annotations

import asyncio
from pathlib import Path
from typing import Any, Callable, Optional, Union

from .context_manager import ContextManager
from .models import ActionPlan, ActionStep, ToolResult
from .path_guard import PathGuard
from .planner import PlanParser
from .policy_engine import PolicyEngine
from .providers import LLMProvider, get_provider
from .tool_registry import ExecContext, ToolRegistry


class AgentOrchestrator:
    """Orchestrates conversations, context preparation, plan generation, and policy enforcement."""

    def __init__(
        self,
        tool_registry: Optional[ToolRegistry] = None,
        policy_engine: Optional[PolicyEngine] = None,
        path_guard: Optional[PathGuard] = None,
        context_manager: Optional[ContextManager] = None,
    ) -> None:
        self.path_guard = path_guard or PathGuard()
        self.policy_engine = policy_engine or PolicyEngine()
        self.tool_registry = tool_registry or ToolRegistry(
            path_guard=self.path_guard,
            policy_engine=self.policy_engine,
        )
        self.context_manager = context_manager or ContextManager(path_guard=self.path_guard)
        self.parser = PlanParser()

    async def handle(
        self,
        user_text: str,
        ctx: Optional[ExecContext] = None,
        provider_name: str = "gemini",
        config: Optional[dict[str, Any]] = None,
        conversation_history: Optional[list[dict[str, str]]] = None,
        terminal_selection: Optional[str] = None,
        attachments: Optional[list[str | Path]] = None,
        streaming_callback: Optional[Callable[[str, bool], None]] = None,
    ) -> Union[ActionPlan, str]:
        """
        Main entrypoint for processing user assistance requests.

        Args:
            user_text: Prompt or query entered by the user.
            ctx: Execution context metadata.
            provider_name: Active LLM provider (gemini, groq, openrouter, ollama).
            config: Provider credentials and endpoint options.
            conversation_history: Prior chat messages.
            terminal_selection: Selected text in terminal.
            attachments: List of file paths attached to request.
            streaming_callback: Optional callback for streaming tokens.

        Returns:
            ActionPlan | str: Evaluated ActionPlan or textual answer.
        """
        ctx = ctx or ExecContext()
        config = config or {}

        provider: LLMProvider = get_provider(provider_name, config)
        ctx.provider_trust = provider.trust

        # 1. Build messages with safety wrappers and tool definitions
        tools_schema = self.tool_registry.get_all_schemas()
        messages = self.context_manager.build_messages(
            user_text=user_text,
            conversation_history=conversation_history,
            terminal_selection=terminal_selection,
            attachments=attachments,
            provider_trust=provider.trust,
            tools_schema=tools_schema,
        )

        # 2. Call provider
        loop = asyncio.get_running_loop()
        if streaming_callback:
            raw_output = await loop.run_in_executor(
                None,
                provider.complete_stream,
                messages,
                streaming_callback,
                tools_schema,
            )
        else:
            raw_output = await loop.run_in_executor(
                None,
                provider.complete,
                messages,
                tools_schema,
            )

        # 3. Parse output into an ActionPlan
        parsed_result = self.parser.parse(raw_output, provider_name=provider.name)

        # 4. If an ActionPlan was generated, enforce security policies
        if isinstance(parsed_result, ActionPlan):
            evaluated_plan = self.policy_engine.evaluate_plan(parsed_result)
            return evaluated_plan

        return parsed_result

    async def execute_step(
        self,
        step: ActionStep,
        ctx: Optional[ExecContext] = None,
    ) -> ToolResult:
        """
        Execute a single plan step using the ToolRegistry.
        """
        ctx = ctx or ExecContext()
        ctx.plan_id = getattr(ctx, "plan_id", "default")
        ctx.step_id = step.step_id
        ctx.dry_run = getattr(ctx, "dry_run", False)

        params: dict[str, Any] = {}
        if step.tool in {"shell.run", "shell.dry_run"}:
            params = {
                "argv": step.argv,
                "working_directory": step.working_directory,
                "timeout_seconds": step.timeout_seconds,
            }
        elif step.tool == "admin.run_action":
            # Extract action_id and params if available
            action_id = step.argv[0] if step.argv else ""
            params = {"action_id": action_id, "dry_run": ctx.dry_run}
        else:
            params = {"path": step.working_directory or "."}

        return await self.tool_registry.invoke(step.tool, params, ctx=ctx)
