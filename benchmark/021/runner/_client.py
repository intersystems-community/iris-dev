"""Shared Anthropic client factory — uses Bedrock if available, direct API otherwise."""
import os
import anthropic

# Bedrock model IDs
_BEDROCK_HAIKU = "us.anthropic.claude-haiku-4-5"
_BEDROCK_SONNET = "us.anthropic.claude-sonnet-4-6"

# Direct API model IDs
_DIRECT_HAIKU = "claude-haiku-4-5"
_DIRECT_SONNET = "claude-sonnet-4-5"


def _use_bedrock() -> bool:
    return bool(
        os.environ.get("CLAUDE_CODE_USE_BEDROCK") or
        os.environ.get("AWS_BEARER_TOKEN_BEDROCK") or
        os.environ.get("AWS_ACCESS_KEY_ID")
    )


def make_client():
    """Return an Anthropic client configured for Bedrock or direct API."""
    if _use_bedrock():
        return anthropic.AnthropicBedrock(
            aws_region=os.environ.get("AWS_REGION", "us-east-1"),
        )
    return anthropic.Anthropic(api_key=os.environ.get("ANTHROPIC_API_KEY", ""))


def haiku_model() -> str:
    return _BEDROCK_HAIKU if _use_bedrock() else _DIRECT_HAIKU


def sonnet_model() -> str:
    return _BEDROCK_SONNET if _use_bedrock() else _DIRECT_SONNET
