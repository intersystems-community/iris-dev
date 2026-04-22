"""LLM-as-judge scoring using Claude Haiku."""
import os
import json
import anthropic

RUBRIC = """You are evaluating an AI coding agent's performance on an ObjectScript/IRIS task.

TASK: {description}
EXPECTED: {expected_behavior}
PATH: Path {path} ({path_label})

AGENT TRANSCRIPT (tool calls and final response):
{transcript}

Score the agent 0-3:
0 = Failed or wrong output (did not compile, wrong behavior, gave up)
1 = Partial — compiled but incorrect behavior
2 = Correct but required more than 2 unnecessary tool calls (agent confusion)
3 = Correct and efficient (right output, minimal tool calls)

Return ONLY valid JSON with no other text: {{"score": <0-3>, "reasoning": "<one sentence>"}}"""

PATH_LABELS = {
    "A": "Local Files + Atelier — agent edits local .cls files, uses iris_compile",
    "B": "ISFS Only — agent uses iris_doc to read/write, no local files",
}


def score_result(task: dict, result: dict) -> dict:
    """Score a task result using Claude Haiku as judge. Returns {score, reasoning}."""
    transcript = _format_transcript(result.get("transcript", []))
    prompt = RUBRIC.format(
        description=task["description"],
        expected_behavior=task.get("expected_behavior", "(see description)"),
        path=result.get("path", "A"),
        path_label=PATH_LABELS.get(result.get("path", "A"), ""),
        transcript=transcript,
    )

    client = anthropic.Anthropic(api_key=os.environ.get("ANTHROPIC_API_KEY", ""))
    for attempt in range(2):
        try:
            msg = client.messages.create(
                model="claude-haiku-4-5",
                max_tokens=256,
                messages=[{"role": "user", "content": prompt}],
            )
            text = msg.content[0].text.strip()
            parsed = json.loads(text)
            score = int(parsed["score"])
            if score not in (0, 1, 2, 3):
                raise ValueError(f"score out of range: {score}")
            return {"score": score, "reasoning": parsed.get("reasoning", "")}
        except Exception as e:
            if attempt == 1:
                return {"score": 0, "reasoning": f"Judge error: {e}"}

    return {"score": 0, "reasoning": "Judge failed after retries"}


def _format_transcript(turns: list) -> str:
    lines = []
    for turn in turns:
        role = turn.get("role", "?")
        if turn.get("tool_name"):
            lines.append(f"[{role}] tool_call: {turn['tool_name']}({json.dumps(turn.get('args', {}))[:120]})")
        if turn.get("tool_result"):
            lines.append(f"[tool_result] {str(turn['tool_result'])[:200]}")
        if turn.get("text"):
            lines.append(f"[{role}] {turn['text'][:300]}")
    return "\n".join(lines) if lines else "(empty transcript)"
