"""Tool registry and dispatcher for Zashterminal Secure Agent."""

from __future__ import annotations

import inspect
import json
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Awaitable, Callable, Optional

from .admin_tools import AdminTools
from .fs_tools import FSTools
from .models import ToolResult
from .path_guard import PathGuard
from .policy_engine import PolicyEngine
from .shell_tools import run_argv


@dataclass
class ExecContext:
    """Execution context provided during tool invocation."""

    plan_id: str = "default"
    step_id: str = "step_0"
    working_directory: Optional[str] = None
    user_approved: bool = False
    dry_run: bool = False
    provider_trust: str = "remote"  # "local" | "remote"


def _load_default_schemas() -> dict[str, dict[str, Any]]:
    schema_path = (
        Path(__file__).resolve().parent.parent / "data" / "agent_tools_schema.json"
    )
    if not schema_path.exists():
        schema_path = Path("/usr/share/zashterminal/data/agent_tools_schema.json")

    if not schema_path.exists():
        return {}

    try:
        with open(schema_path, "r", encoding="utf-8") as f:
            data = json.load(f)
        return {tool["name"]: tool for tool in data.get("tools", [])}
    except Exception:
        return {}


class ToolRegistry:
    """Central registry and asynchronous dispatcher for all agent tools."""

    def __init__(
        self,
        path_guard: Optional[PathGuard] = None,
        policy_engine: Optional[PolicyEngine] = None,
    ) -> None:
        self.path_guard = path_guard or PathGuard()
        self.policy_engine = policy_engine or PolicyEngine()
        self.fs_tools = FSTools(self.path_guard)
        self.admin_tools = AdminTools()

        self._tools: dict[str, Callable[..., Awaitable[ToolResult]]] = {}
        self._schemas: dict[str, dict[str, Any]] = _load_default_schemas()

        self._register_default_tools()

    def register(
        self,
        name: str,
        handler: Callable[..., Awaitable[ToolResult]],
        schema: Optional[dict[str, Any]] = None,
    ) -> None:
        """Register a new tool handler with optional schema."""
        self._tools[name] = handler
        if schema is not None:
            self._schemas[name] = schema

    def get_tool(self, name: str) -> Optional[Callable[..., Awaitable[ToolResult]]]:
        """Retrieve a tool handler by name."""
        return self._tools.get(name)

    def list_tool_names(self) -> list[str]:
        """List all registered tool names."""
        return list(self._tools.keys())

    def get_all_schemas(self) -> list[dict[str, Any]]:
        """Return schema descriptors for all registered tools."""
        return list(self._schemas.values())

    def _register_default_tools(self) -> None:
        # Filesystem read tools
        self.register("fs.list_directory", self.fs_tools.list_directory)
        self.register("fs.metadata", self.fs_tools.metadata)
        self.register("fs.read_file", self.fs_tools.read_file)
        self.register("fs.search_text", self.fs_tools.search_text)
        self.register("fs.disk_usage", self.fs_tools.disk_usage)

        # Filesystem write tools
        self.register("fs.write_staged_file", self.fs_tools.write_staged_file)
        self.register("fs.propose_edit", self.fs_tools.propose_edit)
        self.register("fs.create_directory", self.fs_tools.create_directory)
        self.register("fs.move_to_trash", self.fs_tools.move_to_trash)

        # Shell execution tools
        async def _shell_run(
            argv: list[str],
            working_directory: Optional[str] = None,
            timeout_seconds: int = 30,
            **_kwargs: Any,
        ) -> ToolResult:
            is_denied, reason = self.policy_engine.check_deny_patterns(argv)
            if is_denied:
                return ToolResult(
                    status="denied",
                    stderr=f"Comando proibido pela política: {reason}",
                    exit_code=1,
                )
            return await run_argv(
                argv=argv,
                working_directory=working_directory,
                timeout_seconds=timeout_seconds,
            )

        async def _shell_dry_run(
            argv: list[str],
            working_directory: Optional[str] = None,
            **_kwargs: Any,
        ) -> ToolResult:
            is_denied, reason = self.policy_engine.check_deny_patterns(argv)
            if is_denied:
                return ToolResult(
                    status="denied",
                    stderr=f"Comando proibido pela política: {reason}",
                    exit_code=1,
                )
            return await run_argv(
                argv=argv,
                working_directory=working_directory,
            )

        self.register("shell.run", _shell_run)
        self.register("shell.dry_run", _shell_dry_run)

        # Admin tool
        async def _admin_run(
            action_id: str,
            params: Optional[dict[str, Any]] = None,
            dry_run: bool = False,
            **_kwargs: Any,
        ) -> ToolResult:
            return await self.admin_tools.run_action(
                action_id=action_id,
                params=params or {},
                dry_run=dry_run,
            )

        self.register("admin.run_action", _admin_run)

    async def invoke(
        self,
        name: str,
        params: dict[str, Any],
        ctx: Optional[ExecContext] = None,
    ) -> ToolResult:
        """
        Asynchronously invoke a registered tool by name with parameters.
        """
        handler = self._tools.get(name)
        if handler is None:
            return ToolResult(
                status="error",
                stderr=f"Ferramenta desconhecida: '{name}'",
                exit_code=1,
            )

        # If plan_id is provided in context and supported by the handler, inject it
        call_params = dict(params)
        if ctx:
            sig = inspect.signature(handler)
            if "plan_id" in sig.parameters and "plan_id" not in call_params:
                call_params["plan_id"] = ctx.plan_id
            if (
                "working_directory" in sig.parameters
                and "working_directory" not in call_params
                and ctx.working_directory
            ):
                call_params["working_directory"] = ctx.working_directory
            if "dry_run" in sig.parameters and "dry_run" not in call_params and ctx.dry_run:
                call_params["dry_run"] = ctx.dry_run

        try:
            return await handler(**call_params)
        except TypeError as e:
            return ToolResult(
                status="error",
                stderr=f"Erro de parâmetros para '{name}': {e}",
                exit_code=1,
            )
        except Exception as e:
            return ToolResult(
                status="error",
                stderr=f"Erro ao executar '{name}': {e}",
                exit_code=1,
            )
