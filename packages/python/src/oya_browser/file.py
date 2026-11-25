"""Files as task values, as in the TypeScript SDK's file.ts.

    browser.ask("Attach my resume", data={"resume": file("./cv.pdf")})

The bytes ride inline in the run request, so the agent's upload_file tool can put them into a page's file input.
`secrets` cannot hold one: a file is never typed through a placeholder, so there is nothing to hide.
"""

import base64
import os
from pathlib import Path
from typing import Any

from ._constants import BYTES_PER_MB, MAX_FILE_MB

MAX_FILE_BYTES = MAX_FILE_MB * BYTES_PER_MB
"""Bigger than this and the server would refuse the body anyway."""

# The same table as file.ts rather than the mimetypes module, whose answers depend on the machine's own mime.types.
MIME = {
    "pdf": "application/pdf",
    "png": "image/png",
    "jpg": "image/jpeg",
    "jpeg": "image/jpeg",
    "gif": "image/gif",
    "webp": "image/webp",
    "svg": "image/svg+xml",
    "heic": "image/heic",
    "txt": "text/plain",
    "csv": "text/csv",
    "json": "application/json",
    "xml": "application/xml",
    "html": "text/html",
    "doc": "application/msword",
    "docx": "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    "xls": "application/vnd.ms-excel",
    "xlsx": "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    "ppt": "application/vnd.ms-powerpoint",
    "pptx": "application/vnd.openxmlformats-officedocument.presentationml.presentation",
    "zip": "application/zip",
}
"""Enough to keep a site's accept="" filter happy. Anything else is application/octet-stream."""


def _mime(name: str) -> str:
    return MIME.get(name.rsplit(".", 1)[-1].lower() if "." in name else "", "application/octet-stream")


def file(source: str | os.PathLike[str] | bytes, name: str | None = None, type: str | None = None) -> dict[str, Any]:
    """A file for `data`: a path on disk, or raw bytes. `name` is what the site sees; `type` overrides the MIME type.

    Raises ValueError for a file over MAX_FILE_BYTES, saying how big it is, before anything is sent.
    """
    if isinstance(source, bytes):
        data, suggested = source, None
    else:
        data, suggested = Path(source).read_bytes(), Path(source).name
    filename = name or suggested or "file"
    if len(data) > MAX_FILE_BYTES:
        size = round(len(data) / BYTES_PER_MB)
        raise ValueError(f"{filename} is {size}MB; the limit for a task file is {MAX_FILE_MB}MB.")
    return {"file": filename, "type": type or _mime(filename), "b64": base64.b64encode(data).decode("ascii")}
