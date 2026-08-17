"""Plan parser and validator for converting LLM output into ActionPlan instances."""

from __future__ import annotations

import json
import re
import uuid
from typing import Optional, Union

from .models import ActionPlan, ActionStep, RiskLevel


_JSON_BLOCK_PATTERN = re.compile(r"```(?:json)?\s*(\{[\s\S]*?\})\s*```", re.IGNORECASE)


def extract_json_object(raw_text: str) -> Optional[dict]:
    """Attempt to extract a JSON dictionary from raw model text."""
    if not raw_text or not raw_text.strip():
        return None

    cleaned = raw_text.strip()

    # 1. Direct JSON parse
    try:
        data = json.loads(cleaned)
        if isinstance(data, dict):
            return data
    except Exception:
        pass

    # 2. Extract from markdown code fence
    match = _JSON_BLOCK_PATTERN.search(cleaned)
    if match:
        try:
            data = json.loads(match.group(1))
            if isinstance(data, dict):
                return data
        except Exception:
            pass

    # 3. Find outermost curly braces { ... }
    first_brace = cleaned.find("{")
    last_brace = cleaned.rfind("}")
    if first_brace != -1 and last_brace != -1 and last_brace > first_brace:
        try:
            data = json.loads(cleaned[first_brace : last_brace + 1])
            if isinstance(data, dict):
                return data
        except Exception:
            pass

    return None


class PlanParser:
    """Parses and validates LLM output into structured ActionPlan objects."""

    @staticmethod
    def parse(
        raw_text: str,
        provider_name: str = "",
    ) -> Union[ActionPlan, str]:
        """
        Parse raw model response into an ActionPlan or return pure text fallback.

        Args:
            raw_text: Model output text.
            provider_name: Identifier of the provider.

        Returns:
            ActionPlan if a valid plan structure was decoded, or fallback string.
        """
        json_data = extract_json_object(raw_text)
        if json_data is None:
            return raw_text.strip()

        # Handle legacy OnyxSH format: {"reply": "...", "commands": ["..."]}
        if "reply" in json_data and "steps" not in json_data:
            summary = json_data.get("reply", "")
            raw_commands = json_data.get("commands", [])
            steps = []
            for i, cmd in enumerate(raw_commands):
                cmd_str = cmd if isinstance(cmd, str) else cmd.get("command", "")
                if cmd_str:
                    argv = [tok for tok in cmd_str.split() if tok]
                    steps.append(
                        ActionStep(
                            step_id=f"step_{i+1}",
                            tool="shell.run",
                            argv=argv,
                            description=f"Executar comando: {cmd_str}",
                            risk=RiskLevel.USER_WRITE,
                        )
                    )
            return ActionPlan(
                plan_id=f"plan_{uuid.uuid4().hex[:8]}",
                intent="Execução sugerida",
                summary=summary,
                steps=steps,
                provider=provider_name,
            )

        # Standard ActionPlan schema format
        try:
            if "plan_id" not in json_data:
                json_data["plan_id"] = f"plan_{uuid.uuid4().hex[:8]}"
            if "provider" not in json_data:
                json_data["provider"] = provider_name

            plan = ActionPlan.from_dict(json_data)
            return plan
        except Exception:
            # If ActionPlan schema validation fails, fallback to clean text
            return raw_text.strip()
