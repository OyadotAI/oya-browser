"""Oya.signup(): solve the server's puzzle, get a key, and save it where `oya login` would."""

import hashlib
import json

import httpx
import pytest

from oya_browser import AsyncOya, Oya
from oya_browser.signup import solve

BASE = "https://oya.test"
ANSWER = {"api_key": "oya_new", "claim_url": "https://oya.test/claim/x", "cloud_browsers": False}


def server(fake):
    return fake(
        {
            ("GET", "/api/auth/agent/challenge"): {"challenge": "abc", "difficulty": 2},
            ("POST", "/api/auth/agent/signup"): ANSWER,
        }
    )


def transport(f):
    return httpx.Client(base_url=BASE, transport=httpx.MockTransport(f))


def test_the_nonce_makes_the_hash_start_with_the_zeros_asked_for():
    nonce = solve("abc", 2)
    assert hashlib.sha256(f"abc{nonce}".encode()).hexdigest().startswith("00")


def test_signup_sends_the_email_and_the_solved_puzzle_and_returns_the_key(fake):
    f = server(fake)
    result = Oya.signup("ada@example.com", base_url=BASE, client=transport(f))
    assert result == {"apiKey": "oya_new", "claimUrl": ANSWER["claim_url"], "cloudBrowsers": False, "saved": True}
    body = f.calls[1][2]
    assert (body["email"], body["challenge"], body["nonce"]) == ("ada@example.com", "abc", solve("abc", 2))


def test_signup_saves_the_key_owner_only_where_a_later_oya_finds_it(fake, tmp_path):
    Oya.signup("ada@example.com", base_url=BASE, client=transport(server(fake)))
    path = tmp_path / "config.json"
    assert json.loads(path.read_text()) == {"apiKey": "oya_new", "baseUrl": BASE}
    assert path.stat().st_mode & 0o777 == 0o600
    assert Oya(base_url=BASE)._http.origin.api_key == "oya_new"


def test_signup_never_replaces_a_key_already_saved(fake, tmp_path):
    (tmp_path / "config.json").write_text(json.dumps({"apiKey": "oya_old", "baseUrl": BASE}))
    result = Oya.signup("ada@example.com", base_url=BASE, client=transport(server(fake)))
    assert result["saved"] is False
    assert json.loads((tmp_path / "config.json").read_text())["apiKey"] == "oya_old"


def test_save_false_writes_nothing(fake, tmp_path):
    result = Oya.signup("ada@example.com", base_url=BASE, save=False, client=transport(server(fake)))
    assert result["saved"] is False and not (tmp_path / "config.json").exists()


def test_signup_sends_no_real_key(fake, monkeypatch):
    monkeypatch.setenv("OYA_API_KEY", "oya_secret")
    f = server(fake)
    Oya.signup("ada@example.com", base_url=BASE, save=False, client=transport(f))
    assert all("oya_secret" not in c[3]["Authorization"] for c in f.calls)


async def test_async_signup_does_the_same(fake):
    f = server(fake)
    client = httpx.AsyncClient(base_url=BASE, transport=httpx.MockTransport(f))
    result = await AsyncOya.signup("ada@example.com", base_url=BASE, save=False, client=client)
    assert result["apiKey"] == "oya_new"


def test_a_failed_signup_raises_and_saves_nothing(fake, tmp_path):
    from oya_browser import OyaError

    f = fake(
        {
            ("GET", "/api/auth/agent/challenge"): {"challenge": "abc", "difficulty": 1},
            ("POST", "/api/auth/agent/signup"): httpx.Response(429, json={"error": "Too many signups"}),
        }
    )
    with pytest.raises(OyaError, match="Too many signups"):
        Oya.signup("ada@example.com", base_url=BASE, client=transport(f))
    assert not (tmp_path / "config.json").exists()
