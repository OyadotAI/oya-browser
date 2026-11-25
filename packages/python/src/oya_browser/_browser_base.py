"""What Browser and AsyncBrowser share: what a browser knows about itself, its links, and the browser-use hand-off."""

from typing import Any

from . import _ops as ops
from ._http import AsyncHttp, Http

NO_CDP = "This browser has no CDP endpoint (its provider does not offer one), so browser-use cannot drive it."


def browser_use(cdp_url: str | None) -> Any:
    """A browser_use.Browser on `cdp_url`; browser-use is an optional extra, imported only when asked for."""
    if not cdp_url:
        raise ValueError(NO_CDP)
    try:
        from browser_use import Browser as BrowserUseBrowser
    except ImportError as e:
        raise ImportError('browser-use is not installed: pip install "oya-browser[browser-use]"') from e
    return BrowserUseBrowser(cdp_url=cdp_url)


class BrowserBase:
    """What every browser knows about itself."""

    def __init__(self, http: Http | AsyncHttp, info: dict[str, Any], auto_captcha: bool) -> None:
        self.id: str = info["id"]
        self.provider: str = info.get("provider") or "cdp"
        self.persona: str = info.get("persona") or "default"
        self.cdp_url: str | None = info.get("cdpUrl")
        """Where Playwright, Puppeteer or browser-use connect; single use, see fresh_cdp_url()."""
        self.reused: bool = info.get("reused") is True
        """True when start() handed over a browser that was already running, such as the desktop app."""
        self._auto_captcha = auto_captcha
        self._base_url = http.origin.base_url

    def live_view_url(self) -> str:
        """Watch it work: the console, opened on this browser. Carries no credential; frames are live_stream_url()."""
        return ops.live_view_url(self._base_url, self.id)

    def __repr__(self) -> str:
        return f"<{type(self).__name__} {self.id} on {self.provider}>"
