"""Capture real fzf screens with fictional sessions for the public README.

Build with `cargo build --release`, then run:
uv run --with pillow --with pexpect --with pyte python scripts/capture-demo.py
"""

import datetime
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time

import pexpect
from PIL import Image, ImageDraw, ImageFont
import pyte


ROOT = Path(__file__).resolve().parents[1]
BINARY = ROOT / "target/release/agent-session-hub"
DESTINATION = ROOT / "docs/images"
FONT_PATH = subprocess.check_output(
    ["fc-match", "-f", "%{file}", "monospace"], text=True
).strip()
FONT = ImageFont.truetype(FONT_PATH, 15)
CELL_WIDTH = round(FONT.getlength("M"))
CELL_HEIGHT = 23
COLORS = {
    "black": "#111827", "red": "#f87171", "green": "#86efac",
    "brown": "#fcd34d", "blue": "#93c5fd", "magenta": "#d8b4fe",
    "cyan": "#67e8f9", "white": "#e5e7eb", "default": "#d5dde8",
}


def color(value, default):
    if value == "default":
        return default
    return COLORS.get(value, f"#{value}" if len(value) == 6 else default)


def save_screen(screen, command, filename):
    padding, top = 22, 58
    image = Image.new("RGB", (screen.columns * CELL_WIDTH + padding * 2,
                              screen.lines * CELL_HEIGHT + top + padding), "#0d1420")
    draw = ImageDraw.Draw(image)
    draw.rounded_rectangle((1, 1, image.width - 2, image.height - 2), radius=12,
                           outline="#334155", width=1)
    for index, fill in enumerate(["#f87171", "#fbbf24", "#4ade80"]):
        draw.ellipse((22 + index * 22, 20, 33 + index * 22, 31), fill=fill)
    draw.text((108, 16), command, font=FONT, fill="#94a3b8")
    draw.line((1, 48, image.width - 2, 48), fill="#253246")
    for row in range(screen.lines):
        for column in range(screen.columns):
            cell = screen.buffer[row][column]
            x, y = padding + column * CELL_WIDTH, top + row * CELL_HEIGHT
            fg, bg = color(cell.fg, "#d5dde8"), color(cell.bg, "#0d1420")
            if cell.reverse:
                fg, bg = bg, fg
            if cell.bg != "default" or cell.reverse:
                draw.rectangle((x, y, x + CELL_WIDTH, y + CELL_HEIGHT), fill=bg)
            draw.text((x, y), cell.data, font=FONT, fill=fg)
    image.save(DESTINATION / filename, optimize=True)


def collect(child, stream, seconds=0.7):
    until = time.monotonic() + seconds
    while time.monotonic() < until:
        try:
            stream.feed(child.read_nonblocking(65536, timeout=0.05))
        except pexpect.TIMEOUT:
            pass
        except pexpect.EOF:
            break


def capture(env, args, filename, expanded=False):
    lines = 22 if args[0] == "browse" else 11
    screen = pyte.Screen(152, lines)
    stream = pyte.Stream(screen)
    child = pexpect.spawn(str(BINARY), ["--provider", "codex", *args], env=env,
                          encoding="utf-8", codec_errors="replace", dimensions=(lines, 152))
    try:
        collect(child, stream, 1.2)
        if expanded:
            # The first row is the newest workspace; its first chat has agents.
            child.send("\x1b[B")
            collect(child, stream)
            child.send("\x01")
            collect(child, stream, 1.2)
        save_screen(screen, "csx " + " ".join(args), filename)
        child.send("\x1b")
        collect(child, stream)
    finally:
        child.close(force=True)


def main():
    DESTINATION.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="ash-demo-") as temporary:
        root = Path(temporary)
        sessions = root / "sessions"
        sessions.mkdir()
        now = datetime.datetime.now(datetime.timezone.utc)

        def session(identity, project, title, mib, days=0, root_id=None, nickname=None):
            timestamp = now - datetime.timedelta(days=days, minutes=10)
            payload = {"id": identity, "cwd": project, "timestamp": timestamp.isoformat()}
            if root_id:
                payload.update(session_id=root_id, source={"subagent": {"thread_spawn": {
                    "parent_thread_id": root_id, "depth": 1,
                    "agent_path": f"/root/{nickname.lower()}", "agent_nickname": nickname,
                }}})
            entries = [{"type": "session_meta", "payload": payload},
                       {"type": "event_msg", "payload": {"type": "user_message", "message": title}}]
            path = sessions / f"{identity}.jsonl"
            with path.open("w") as handle:
                handle.write("\n".join(json.dumps(entry) for entry in entries) + "\n")
                handle.truncate(int(mib * 1024 * 1024))
            os.utime(path, (timestamp.timestamp(), timestamp.timestamp()))

        session("atlas-auth", "/workspace/atlas", "Add passkey sign-in and recovery flows", 6.4)
        session("atlas-review", "/workspace/atlas", "Review the account recovery edge cases", 2.1,
                root_id="atlas-auth", nickname="Reviewer")
        session("atlas-tests", "/workspace/atlas", "Test passkey registration and recovery", 1.8,
                root_id="atlas-auth", nickname="Tester")
        session("atlas-api", "/workspace/atlas", "Refactor the account API validation", 3.2, days=2)
        session("orbit-release", "/workspace/orbit", "Prepare release notes and migration guide", 4.6, days=3)
        session("orbit-design", "/workspace/orbit", "Polish the settings screen", 1.1, days=5)
        session("old-agent", "/workspace/orbit", "Check compatibility with previous versions", 22, days=45,
                root_id="orbit-release", nickname="Compatibility")
        scan = "/tmp/codex-security-scans-demo/dependency/scan-2026-09-12/output"
        session("scan-root", scan, "Review dependency input handling", 32, days=40)
        session("scan-agent", scan, "Validate the scan findings", 54, days=40,
                root_id="scan-root", nickname="Validator")
        theme = ("--color=fg:#d5dde8,bg:#0d1420,hl:#67e8f9,fg+:#e2e8f0,bg+:#18344a,"
                 "hl+:#67e8f9,info:#94a3b8,prompt:#67e8f9,pointer:#67e8f9,"
                 "marker:#86efac,header:#94a3b8,border:#334155")
        env = dict(os.environ, TERM="xterm-256color", FZF_DEFAULT_OPTS=theme,
                   CODEX_SESSION_HUB_SESSION_ROOT=str(sessions),
                   CODEX_SESSION_HUB_CONFIG_ROOT=str(root / "config"),
                   CODEX_SESSION_HUB_CACHE_ROOT=str(root / "cache"))
        env.pop("FZF_DEFAULT_OPTS_FILE", None)
        env.pop("NO_COLOR", None)
        capture(env, ["browse"], "session-picker.png", expanded=True)
        capture(env, ["cleanup"], "storage-cleanup.png")


if __name__ == "__main__":
    main()
