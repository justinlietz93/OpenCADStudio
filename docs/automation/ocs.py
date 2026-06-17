"""Thin Python client for the Open CAD Studio headless automation server.

Launches `OpenCADStudio --serve` and talks to it over a line-based JSON protocol
(one request object per line on stdin, one response per line on stdout). There
is nothing to compile or maintain on the Python side — every method is one JSON
message; the real work is Open CAD Studio's own command system.

    from ocs import Ocs

    with Ocs(binary="OpenCADStudio") as ocs:
        ocs.open("plan.dwg")
        ocs.run("LAYER Walls")
        print(ocs.entities())          # {"total": 42, "by_type": {...}}
        ocs.save("plan_out.dwg")

Each call returns the parsed response dict and raises `OcsError` on `ok: false`.
"""

from __future__ import annotations

import json
import subprocess
from typing import Any, Optional


class OcsError(RuntimeError):
    """Raised when the server replies with `{"ok": false, ...}`."""


class Ocs:
    def __init__(self, binary: str = "OpenCADStudio") -> None:
        self.proc = subprocess.Popen(
            [binary, "--serve"],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            text=True,
            bufsize=1,
        )
        self._read()  # the {"ready": true} greeting

    # ── protocol ────────────────────────────────────────────────────────────
    def _read(self) -> dict[str, Any]:
        line = self.proc.stdout.readline()
        if not line:
            raise OcsError("server closed the connection")
        return json.loads(line)

    def _send(self, **req: Any) -> dict[str, Any]:
        self.proc.stdin.write(json.dumps(req) + "\n")
        self.proc.stdin.flush()
        resp = self._read()
        if not resp.get("ok", False):
            raise OcsError(resp.get("error", "unknown error"))
        return resp

    # ── operations ──────────────────────────────────────────────────────────
    def new(self) -> dict[str, Any]:
        """Start an empty document."""
        return self._send(op="new")

    def open(self, path: str) -> dict[str, Any]:
        """Load a DWG/DXF drawing."""
        return self._send(op="open", path=path)

    def run(self, cmd: str) -> dict[str, Any]:
        """Run a command through Open CAD Studio's command system."""
        return self._send(op="run", cmd=cmd)

    def entities(self) -> dict[str, Any]:
        """Total entity count and a breakdown by type."""
        return self._send(op="entities")

    def save(self, path: Optional[str] = None) -> dict[str, Any]:
        """Write the document (defaults to the opened/last-saved path)."""
        return self._send(op="save", path=path)

    # ── lifecycle ───────────────────────────────────────────────────────────
    def close(self) -> None:
        if self.proc.stdin:
            self.proc.stdin.close()
        try:
            self.proc.wait(timeout=5)
        except subprocess.TimeoutExpired:
            self.proc.kill()

    def __enter__(self) -> "Ocs":
        return self

    def __exit__(self, *_exc: object) -> None:
        self.close()
