"""Where the key and the address come from: the option, then OYA_API_KEY / OYA_BASE_URL, then ~/.oya/config.json.

The same order as the TypeScript SDK and the `oya` CLI, so a key saved by `oya login` works here unchanged.
A saved key is only used for the deployment it was saved for, so a staging key never goes to production.
"""

import json
import os
import uuid
from dataclasses import dataclass
from pathlib import Path
from urllib.parse import urlsplit

from ._constants import DEFAULT_BASE_URL, JSON_INDENT, PRIVATE_DIR, PRIVATE_FILE

NO_KEY = "No API key. Get one at https://oyabrowser.com, then set OYA_API_KEY or pass api_key=..."


@dataclass(frozen=True)
class Origin:
    """The key and the address the client calls."""

    api_key: str
    base_url: str


def config_file() -> Path:
    """The CLI's config file: OYA_CONFIG_HOME, else ~/.oya, then config.json."""
    return Path(os.environ.get("OYA_CONFIG_HOME") or str(Path.home() / ".oya")) / "config.json"


def _saved() -> dict[str, str]:
    """The CLI's saved config, or nothing when there is none or it does not parse."""
    try:
        data = json.loads(config_file().read_text())
    except (OSError, ValueError):
        return {}
    return {k: v for k, v in data.items() if isinstance(v, str)} if isinstance(data, dict) else {}


DEFAULT_PORTS = {"http": 80, "https": 443}


def _deployment(url: str) -> tuple[str, str, int | None, str] | None:
    """The deployment a URL belongs to: scheme, host, non-default port and path."""
    parts = urlsplit(url.strip())
    scheme = parts.scheme.lower()
    if scheme not in DEFAULT_PORTS or not parts.hostname:
        return None
    try:
        port = parts.port
    except ValueError:
        return None
    effective_port = None if port in (None, DEFAULT_PORTS[scheme]) else port
    return (scheme, parts.hostname.lower(), effective_port, parts.path.rstrip("/"))


def save_key(api_key: str, base_url: str) -> bool:
    """Saves a key where `oya login` would, unless one is there already, owner-only; True when it wrote."""
    if _saved().get("apiKey"):
        return False
    path = config_file()
    path.parent.mkdir(mode=PRIVATE_DIR, parents=True, exist_ok=True)
    text = json.dumps({**_saved(), "apiKey": api_key, "baseUrl": base_url}, indent=JSON_INDENT)
    _write_private(path, text)
    return True


def _write_private(path: Path, text: str) -> None:
    """Writes `text` to `path` readable by its owner only, whatever was there before.

    Opening the file itself with O_TRUNC would keep the mode of a file that already exists (a config left
    world-readable stays so) and would follow a symlink planted at the path. So the key goes to a new file,
    created exclusively and not through a link, and is renamed over the config in one step.
    """
    tmp = path.with_name(f".{path.name}.{os.getpid()}-{uuid.uuid4()}.tmp")
    fd = os.open(tmp, os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os, "O_NOFOLLOW", 0), PRIVATE_FILE)
    try:
        with os.fdopen(fd, "w") as f:
            f.write(text)
        os.replace(tmp, path)
    except BaseException:
        tmp.unlink(missing_ok=True)
        raise


def resolve(api_key: str | None = None, base_url: str | None = None) -> Origin:
    """The key and address to use; raises without a key."""
    saved = _saved()
    address = (base_url or os.environ.get("OYA_BASE_URL") or saved.get("baseUrl") or DEFAULT_BASE_URL).rstrip("/")
    saved_base = saved.get("baseUrl") or DEFAULT_BASE_URL
    target = _deployment(address)
    saved_key = saved.get("apiKey") if target is not None and _deployment(saved_base) == target else None
    key = (api_key or os.environ.get("OYA_API_KEY") or saved_key or "").strip()
    if not key:
        raise ValueError(NO_KEY)
    return Origin(api_key=key, base_url=address)
