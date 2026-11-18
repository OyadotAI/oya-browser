"""oya-browser: real browsers for AI agents, from Python.

    from oya_browser import Oya

    with Oya() as oya, oya.browser.start() as browser:
        browser.goto("https://example.com")
        print(browser.ask("What is the page title?"))

With browser-use (pip install "oya-browser[browser-use]"):

    agent = Agent(task, llm, browser=browser.for_browser_use())
"""

from .browser import AsyncBrowser, Browser
from .client import AsyncOya, Oya
from .errors import OyaError
from .file import MAX_FILE_BYTES, file
from .run import AsyncRun, Run

__all__ = ["MAX_FILE_BYTES", "AsyncBrowser", "AsyncOya", "AsyncRun", "Browser", "Oya", "OyaError", "Run", "file"]
__version__ = "0.2.0"
