"""Oya.signup / AsyncOya.signup: an agent's own key, with no person or dashboard, as in the TypeScript SDK's agent.ts.

The server sets a proof-of-work puzzle so nobody mints keys in bulk; this solves it, so no agent has to. The key is
saved to ~/.oya/config.json (unless one is there already), where a later Oya() finds it.
"""

import asyncio
import hashlib
import itertools
from typing import Any

import httpx

from ._config import resolve, save_key
from ._http import AsyncHttp, Http

NO_KEY = "signup"
"""Stands in for a key on the two signup calls, which take none."""


def solve(challenge: str, difficulty: int) -> str:
    """A nonce that makes sha256(challenge + nonce) start with `difficulty` zero hex digits. About a million tries."""
    zeros = "0" * difficulty
    for nonce in itertools.count():
        if hashlib.sha256(f"{challenge}{nonce}".encode()).hexdigest().startswith(zeros):
            return str(nonce)
    raise AssertionError("unreachable")


def _body(email: str, puzzle: dict[str, Any], nonce: str) -> dict[str, Any]:
    return {"email": email, "challenge": puzzle["challenge"], "nonce": nonce}


def _result(answer: dict[str, Any], base_url: str, save: bool) -> dict[str, Any]:
    """{apiKey, claimUrl, cloudBrowsers, saved}: send claimUrl to the person, it unlocks cloud browsers."""
    saved = save and save_key(answer["api_key"], base_url)
    return {
        "apiKey": answer["api_key"],
        "claimUrl": answer["claim_url"],
        "cloudBrowsers": answer["cloud_browsers"],
        "saved": saved,
    }


def signup(email: str, base_url: str | None = None, save: bool = True, client: httpx.Client | None = None) -> Any:
    """Signs an agent up for its own key, on behalf of the person whose email it gives."""
    http = Http(resolve(NO_KEY, base_url), client)
    try:
        puzzle = http.request("GET", "/api/auth/agent/challenge")
        nonce = solve(puzzle["challenge"], int(puzzle["difficulty"]))
        answer = http.request("POST", "/api/auth/agent/signup", _body(email, puzzle, nonce))
    finally:
        if client is None:
            http.close()
    return _result(answer, http.origin.base_url, save)


async def async_signup(
    email: str, base_url: str | None = None, save: bool = True, client: httpx.AsyncClient | None = None
) -> Any:
    """signup(), with asyncio; the puzzle is solved in a thread so the event loop keeps running."""
    http = AsyncHttp(resolve(NO_KEY, base_url), client)
    try:
        puzzle = await http.request("GET", "/api/auth/agent/challenge")
        nonce = await asyncio.to_thread(solve, puzzle["challenge"], int(puzzle["difficulty"]))
        answer = await http.request("POST", "/api/auth/agent/signup", _body(email, puzzle, nonce))
    finally:
        if client is None:
            await http.aclose()
    return _result(answer, http.origin.base_url, save)
