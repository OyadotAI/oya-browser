"""Every number the SDK uses, by name, in seconds: the same budgets as the TypeScript SDK's constants.ts."""

DEFAULT_BASE_URL = "https://oyabrowser.com"
"""The hosted control plane, used when no option, environment variable or saved config names one."""

DEFAULT_TIMEOUT_S = 60.0
"""A plain API call."""
START_TIMEOUT_S = 120.0
"""Starting a browser: the start call itself, before any wait for a cloud browser to dial in."""
READY_TIMEOUT_S = 120.0
"""How long a starting cloud browser has to dial in."""
READY_POLL_S = 2.0
"""How often the fleet is checked while a cloud browser is starting."""
NAVIGATE_TIMEOUT_S = 120.0
"""A navigation, which waits for the page to load."""
CHALLENGE_TIMEOUT_S = 180.0
"""Solving a CAPTCHA or completing MFA."""
AGENT_TIMEOUT_S = 600.0
"""An agent task or a playbook replay; the server keeps the connection alive meanwhile."""
PLAYBOOK_TIMEOUT_S = 120.0
"""Saving a playbook."""
STOP_TIMEOUT_S = 60.0
"""Stopping a browser, which destroys a cloud sandbox."""
WAIT_FOR_DEFAULT_S = 30.0
"""How long wait_for() waits for a selector by default."""
WAIT_FOR_GRACE_S = 5.0
"""Extra request time on top of wait_for()'s own timeout, so the server answers first."""
MS_PER_SECOND = 1000
"""The API takes wait_for()'s timeout in milliseconds."""
AIMED_SCROLL_AMOUNT = 500
"""Pixels an aimed scroll (scroll(at=...)) moves when no amount is given."""
SHARE_EXPIRES_S = 3600
"""How long a share_url() link works by default."""
RUN_POLL_S = 2.0
"""How often a submitted run is polled by default."""
MAX_RUN_POLL_ERRORS = 5
"""Failed polls in a row before a run is reported as failed."""
DESKTOP_TIMEOUT_S = 300.0
"""How long desktop.connect() waits for a person to click Connect (and allow the keychain prompt)."""
DESKTOP_PROVIDER = "oya-desktop"
"""The provider the server names the Oya desktop app by."""
DESKTOP_DOWNLOAD_URL = "https://oyabrowser.com/#download"
"""Where the desktop app is downloaded."""
BYTES_PER_MB = 1_048_576
"""Bytes in a megabyte, for file limits and messages."""
MAX_FILE_MB = 10
"""Largest task file: bigger and the server would refuse the body anyway."""
PRIVATE_FILE = 0o600
"""Owner read and write only: the saved config holds a key."""
JSON_INDENT = 2
"""Indent of the saved config, as `oya login` writes it."""

STATUS_UNKNOWN = 0
"""A failure that was not an HTTP answer, such as a run that could not be polled."""
STATUS_NOT_FOUND = 404
"""A session not recorded yet, which is fine while a browser starts."""
STATUS_CONFLICT = 409
"""A browser whose creation ended."""
STATUS_UNPROCESSABLE = 422
"""A command that ran and failed."""
STATUS_SERVER_ERROR = 500
"""An agent failure reported without a status."""
STATUS_GATEWAY_TIMEOUT = 504
"""A browser that did not dial in before its budget ran out."""
STATUS_BAD_REQUEST_FLOOR = 400
"""Responses from here up are errors."""
