#!/usr/bin/env python3
"""Validate cloud wiring in rendered deployments (requires kubectl and PyYAML)."""
from pathlib import Path
import subprocess
import yaml

ROOT = Path(__file__).resolve().parents[2]
for environment, host in [
    ("dev", "dev.oyabrowser.com"),
    ("prod", "oyabrowser.com"),
]:
    rendered = subprocess.check_output(
        ["kubectl", "kustomize", str(ROOT / "k8s/overlays" / environment)], text=True
    )
    server = next(doc for doc in yaml.safe_load_all(rendered)
                  if doc.get("kind") == "Deployment" and doc["metadata"]["name"] == "server")
    container = next(c for c in server["spec"]["template"]["spec"]["containers"] if c["name"] == "server")
    env = {item["name"]: item.get("value") for item in container["env"]}
    assert env.get("OYA_PUBLIC_WS_URL") == f"wss://{host}/ws", f"{environment}: missing public callback"
    assert any(item.get("secretRef", {}).get("name") == "app-secrets" for item in container["envFrom"])

    workflow = yaml.safe_load((ROOT / f".github/workflows/deploy-{environment}.yaml").read_text())
    step = next(s for s in workflow["jobs"]["deploy"]["steps"] if s.get("name") == "Apply secrets")
    # Not an exact match: prod sources the snapshot as
    # `${{ needs['register-snapshot'].outputs.snapshot || secrets.DAYTONA_SNAPSHOT }}`,
    # which is still sourced from the secret. Requiring the bare form failed this
    # check on prod for every release, which is why nothing flagged the gap below.
    for name in ["DAYTONA_API_KEY", "DAYTONA_SNAPSHOT", "DAYTONA_API_URL", "DAYTONA_TARGET"]:
        assert "secrets." + name in (step.get("env", {}).get(name) or ""), f"{environment}: {name} not sourced"
        assert f'--from-literal={name}="${{{name}' in step["run"], f"{environment}: {name} not passed to pod secret"
    for name in ["DAYTONA_API_KEY", "DAYTONA_SNAPSHOT"]:
        assert '${' + name + ':?' in step["run"], f"{environment}: missing {name} must fail deployment"

    # app-secrets is rebuilt from this fixed list on every deploy, so a variable
    # absent here can never reach the pod however it is set in the cluster. That
    # is how Slack OAuth shipped dark: the console fell back to asking for a bot
    # token by hand because the server saw no client id. Unlike Daytona these
    # stay optional, a deployment with no Slack app is a supported one.
    # Analytics and ops webhooks are no-ops when unset, so they ship dark the same way.
    for name in ["SLACK_CLIENT_ID", "SLACK_CLIENT_SECRET", "SLACK_SIGNING_SECRET",
                 "POSTHOG_KEY", "POSTHOG_HOST", "SLACK_OPS_WEBHOOK_SIGNUPS", "SLACK_OPS_WEBHOOK_EVENTS",
                 "OYA_RESIDENTIAL_PROXY_URL", "TURNSTILE_SITE_KEY", "TURNSTILE_SECRET_KEY"]:
        assert "secrets." + name in (step.get("env", {}).get(name) or ""), f"{environment}: {name} not sourced"
        assert f'--from-literal={name}="${{{name}' in step["run"], f"{environment}: {name} not passed to pod secret"

    # consoleUrl() derives the Slack live-browser links from this when
    # OYA_CONSOLE_URL is unset, so the scheme swap has to land on a real host.
    assert env["OYA_PUBLIC_WS_URL"].startswith("wss://"), f"{environment}: console URL would derive as http"
    print(f"PASS {environment}: public callback, cloud and Slack secrets reach server")
