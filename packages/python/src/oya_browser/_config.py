"""Where the key and the address come from: the option, then OYA_API_KEY / OYA_BASE_URL, then ~/.oya/config.json.

The same order as the TypeScript SDK and the `oya` CLI, so a key saved by `oya login` works here unchanged.
A saved key is only used for the deployment it was saved for, so a staging key never goes to production.
"""

import json
import os
from dataclasses import dataclass
from pathlib import Path
from urllib.parse import urlsplit

from ._constants import DEFAULT_BASE_URL

NO_KEY = "No API key. Get one at https://oyabrowser.com, then set OYA_API_KEY or pass api_key=..."


@dataclass(frozen=True)
class Origin:
    """The key and the address the client calls."""

    api_key: str
    base_url: str


def _saved() -> dict[str, str]:
    """The CLI's saved config, or nothing when there is none or it does not parse."""
    home = os.environ.get("OYA_CONFIG_HOME") or str(Path.home() / ".oya")
    try:
        data = json.loads((Path(home) / "config.json").read_text())
    except (OSError, ValueError):
        return {}
    return {k: v for k, v in data.items() if isinstance(v, str)} if isinstance(data, dict) else {}


def _host(url: str) -> str:
    """The deployment a URL belongs to: its host, port included."""
    return urlsplit(url).netloc


def resolve(api_key: str | None = None, base_url: str | None = None) -> Origin:
    """The key and address to use; raises without a key."""
    saved = _saved()
    address = (base_url or os.environ.get("OYA_BASE_URL") or saved.get("baseUrl") or DEFAULT_BASE_URL).rstrip("/")
    saved_base = saved.get("baseUrl", DEFAULT_BASE_URL)
    saved_key = saved.get("apiKey") if _host(saved_base) == _host(address) else None
    key = (api_key or os.environ.get("OYA_API_KEY") or saved_key or "").strip()
    if not key:
        raise ValueError(NO_KEY)
    return Origin(api_key=key, base_url=address)
