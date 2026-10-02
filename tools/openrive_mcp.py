#!/usr/bin/env python3
"""Minimal client for the OpenRive MCP server (stdlib only).

OpenRive (https://github.com/UpstandPlatform/OpenRive) is a local Rive editor
that exposes an MCP server over Streamable HTTP. When the app is running it
listens on a random loopback port and answers JSON-RPC at `/api/mcp` (no auth on
loopback). This module discovers that port and speaks just enough MCP to call
tools: initialize -> notifications/initialized -> tools/call.

Used by `tools/ir_to_rive.py`; also handy on its own:

    python3 tools/openrive_mcp.py list
    python3 tools/openrive_mcp.py tools
    python3 tools/openrive_mcp.py call list_templates
"""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys
import urllib.request


class McpError(RuntimeError):
    pass


def _candidate_ports() -> list[int]:
    """Loopback ports held by a `bun`/OpenRive process (macOS/Linux lsof)."""
    if os.environ.get("OPENRIVE_URL"):
        return []
    try:
        out = subprocess.run(
            ["lsof", "-nP", "-iTCP", "-sTCP:LISTEN"],
            capture_output=True,
            text=True,
            timeout=10,
        ).stdout
    except (OSError, subprocess.SubprocessError):
        return []
    ports: list[int] = []
    for line in out.splitlines()[1:]:
        low = line.lower()
        if "bun" not in low and "openrive" not in low:
            continue
        m = re.search(r"127\.0\.0\.1:(\d+)", line)
        if m and int(m.group(1)) not in ports:
            ports.append(int(m.group(1)))
    return ports


def discover_url() -> str:
    """Return the MCP URL, honouring `OPENRIVE_URL` then probing loopback."""
    if url := os.environ.get("OPENRIVE_URL"):
        return url
    probe = {"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
        "protocolVersion": "2025-06-18",
        "capabilities": {},
        "clientInfo": {"name": "lambdaDX-export", "version": "0"},
    }}
    for port in _candidate_ports():
        url = f"http://127.0.0.1:{port}/api/mcp"
        try:
            req = urllib.request.Request(
                url,
                data=json.dumps(probe).encode(),
                headers={
                    "Content-Type": "application/json",
                    "Accept": "application/json, text/event-stream",
                },
                method="POST",
            )
            with urllib.request.urlopen(req, timeout=3) as r:
                body = r.read().decode()
            name = _parse(body).get("result", {}).get("serverInfo", {}).get("name")
            if name == "openrive":
                return url
        except Exception:
            continue
    raise McpError(
        "OpenRive MCP not found. Is OpenRive running? "
        "Set OPENRIVE_URL=http://127.0.0.1:<port>/api/mcp to override."
    )


def _parse(text: str):
    """Decode a JSON or SSE (`data:`) response body."""
    text = text.strip()
    if not text:
        return {}
    for line in text.splitlines():
        if line.startswith("data:"):
            return json.loads(line[5:].strip())
    return json.loads(text)


class OpenRive:
    """A single MCP session against one OpenRive instance."""

    def __init__(self, url: str | None = None):
        self.url = url or discover_url()
        self.session_id: str | None = None
        self._next_id = 1
        self.initialize()

    def _post(self, body: dict) -> dict:
        headers = {
            "Content-Type": "application/json",
            "Accept": "application/json, text/event-stream",
        }
        if self.session_id:
            headers["mcp-session-id"] = self.session_id
        req = urllib.request.Request(
            self.url, data=json.dumps(body).encode(), headers=headers, method="POST"
        )
        with urllib.request.urlopen(req, timeout=60) as r:
            if self.session_id is None:
                self.session_id = r.headers.get("mcp-session-id")
            return _parse(r.read().decode())

    def _request(self, method: str, params=None) -> dict:
        body: dict = {"jsonrpc": "2.0", "id": self._next_id, "method": method}
        self._next_id += 1
        if params is not None:
            body["params"] = params
        obj = self._post(body)
        if "error" in obj:
            raise McpError(f"{method}: {obj['error']}")
        return obj.get("result", {})

    def initialize(self) -> dict:
        result = self._request(
            "initialize",
            {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": {"name": "lambdaDX-export", "version": "0"},
            },
        )
        self._post({"jsonrpc": "2.0", "method": "notifications/initialized"})
        return result

    def tools(self) -> list[dict]:
        return self._request("tools/list").get("tools", [])

    def call(self, name: str, arguments: dict | None = None):
        """Call a tool and return its decoded JSON result (text content)."""
        result = self._request(
            "tools/call", {"name": name, "arguments": arguments or {}}
        )
        if result.get("isError"):
            raise McpError(f"{name}: {result}")
        texts = [c.get("text", "") for c in result.get("content", []) if c.get("type") == "text"]
        joined = "\n".join(texts).strip()
        if not joined:
            return result
        try:
            return json.loads(joined)
        except json.JSONDecodeError:
            return joined


def main(argv: list[str]) -> int:
    if not argv or argv[0] in ("-h", "--help"):
        print(__doc__)
        return 0
    client = OpenRive()
    cmd = argv[0]
    if cmd == "tools":
        for t in client.tools():
            print(f"{t['name']}\t{t.get('description', '')}")
    elif cmd == "list":
        print(json.dumps(client.call("list_projects"), indent=2))
    elif cmd == "url":
        print(client.url)
    elif cmd == "call":
        args = json.loads(argv[2]) if len(argv) > 2 else {}
        print(json.dumps(client.call(argv[1], args), indent=2))
    else:
        print(f"unknown command: {cmd}", file=sys.stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
