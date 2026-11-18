"""Where the key and the address come from: option, then environment, then the key `oya login` saved."""

import json

import pytest

from oya_browser._config import NO_KEY, resolve


def save(tmp_path, **config):
    (tmp_path / "config.json").write_text(json.dumps(config))


def test_the_option_wins_over_the_environment(monkeypatch):
    monkeypatch.setenv("OYA_API_KEY", "env-key")
    assert resolve(api_key="option-key").api_key == "option-key"


def test_the_environment_wins_over_the_saved_key(monkeypatch, tmp_path):
    save(tmp_path, apiKey="saved-key")
    monkeypatch.setenv("OYA_API_KEY", "env-key")
    assert resolve().api_key == "env-key"


def test_the_key_oya_login_saved_is_used_when_nothing_else_names_one(tmp_path):
    save(tmp_path, apiKey="saved-key", baseUrl="https://oyabrowser.com")
    origin = resolve()
    assert (origin.api_key, origin.base_url) == ("saved-key", "https://oyabrowser.com")


def test_a_saved_key_never_goes_to_another_deployment(tmp_path):
    save(tmp_path, apiKey="staging-key", baseUrl="https://staging.example")
    with pytest.raises(ValueError, match="No API key"):
        resolve(base_url="https://oyabrowser.com")


def test_no_key_anywhere_says_where_to_get_one():
    with pytest.raises(ValueError) as e:
        resolve()
    assert str(e.value) == NO_KEY


def test_the_hosted_address_is_the_default():
    assert resolve(api_key="k").base_url == "https://oyabrowser.com"


def test_a_saved_key_is_owner_only_even_over_a_world_readable_config(tmp_path):
    from oya_browser._config import save_key

    path = tmp_path / "config.json"
    path.write_text(json.dumps({"baseUrl": "https://oyabrowser.com"}))
    path.chmod(0o644)
    assert save_key("new-key", "https://oyabrowser.com") is True
    assert path.stat().st_mode & 0o777 == 0o600
    assert json.loads(path.read_text())["apiKey"] == "new-key"


def test_saving_a_key_never_writes_through_a_symlink_at_the_config_path(tmp_path):
    from oya_browser._config import save_key

    elsewhere = tmp_path / "elsewhere.json"
    elsewhere.write_text("{}")
    (tmp_path / "config.json").symlink_to(elsewhere)
    save_key("new-key", "https://oyabrowser.com")
    assert elsewhere.read_text() == "{}"
    assert not (tmp_path / "config.json").is_symlink()
    assert list(tmp_path.glob(".config.json.*.tmp")) == []
