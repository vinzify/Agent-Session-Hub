"""Exercise real picker startup, search, expansion and cancellation in a PTY.

All session data is fictional and isolated in a temporary directory.
Requires pexpect and pyte; see docs/architecture.md for invocation.
"""

import argparse
import json
import os
from pathlib import Path
import tempfile
import time

import pexpect
import pyte


class Picker:
    def __init__(self, binary, env, args):
        self.screen = pyte.Screen(180, 32)
        self.stream = pyte.Stream(self.screen)
        self.transcript = ""
        self.child = pexpect.spawn(
            str(binary), ["--provider", "codex", *args], env=env,
            encoding="utf-8", codec_errors="replace", dimensions=(32, 180),
        )

    def read(self):
        try:
            data = self.child.read_nonblocking(65536, timeout=0.05)
            self.transcript += data
            self.stream.feed(data)
        except pexpect.TIMEOUT:
            pass
        except pexpect.EOF:
            return False
        return True

    def wait_for(self, predicate, description):
        deadline = time.monotonic() + 12
        while time.monotonic() < deadline:
            alive = self.read()
            text = "\n".join(self.screen.display)
            if predicate(text):
                return
            if not alive:
                break
        raise AssertionError(f"{description}\n{text}\n{self.transcript[-1000:]}")

    def finish(self, key, expected_selection=None):
        self.child.send(key)
        deadline = time.monotonic() + 12
        while time.monotonic() < deadline:
            if not self.read():
                break
        else:
            raise AssertionError("Picker did not exit")
        self.child.close()
        assert self.child.exitstatus == 0, self.transcript[-2000:]
        assert "Broken pipe" not in self.transcript, self.transcript[-2000:]
        assert "unsupported key" not in self.transcript, self.transcript[-2000:]
        assert "unknown action" not in self.transcript, self.transcript[-2000:]
        if expected_selection:
            assert expected_selection in self.transcript, self.transcript[-2000:]

    def close(self):
        self.child.close(force=True)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--fzf", type=Path, required=True)
    args = parser.parse_args()
    binary = args.binary.resolve()
    fzf = args.fzf.resolve()

    with tempfile.TemporaryDirectory(prefix="ash-fzf-compat-") as temporary:
        root = Path(temporary)
        sessions = root / "sessions"
        sessions.mkdir()
        bin_dir = root / "bin"
        bin_dir.mkdir()
        (bin_dir / "fzf").symlink_to(fzf)

        def session(identity, project, title, updated, parent=None):
            meta = {"id": identity, "cwd": project, "timestamp": "2026-01-01T10:00:00Z"}
            if parent:
                meta.update(session_id=parent, source={"subagent": {"thread_spawn": {
                    "parent_thread_id": parent, "depth": 1,
                    "agent_path": "/root/audit", "agent_nickname": "AuditAgent",
                }}})
            records = [
                {"type": "session_meta", "payload": meta},
                {"type": "event_msg", "payload": {"type": "user_message", "message": title}},
            ]
            path = sessions / f"{identity}.jsonl"
            path.write_text("\n".join(json.dumps(record) for record in records) + "\n")
            os.utime(path, (updated, updated))

        now = time.time()
        session("root-session", "/demo/alpha", "ROOT_ONE_CANARY review account settings", now)
        session("audit-agent", "/demo/alpha", "Audit the account settings implementation", now, "root-session")
        session("second-session", "/demo/beta", "SECOND_PROJECT_CANARY prepare release notes", now - 60)
        scan = "/tmp/codex-security-scans-fixture/repo/run/output"
        session("scan-session", scan, "SCAN_ROOT_CANARY inspect generated reports", now - 120)
        session("scan-agent", scan, "Inspect the generated scan findings", now - 120, "scan-session")

        env = dict(os.environ, TERM="xterm-256color", SHELL="/bin/sh", FZF_DEFAULT_OPTS="",
                   CODEX_SESSION_HUB_SESSION_ROOT=str(sessions),
                   CODEX_SESSION_HUB_CONFIG_ROOT=str(root / "config"),
                   CODEX_SESSION_HUB_CACHE_ROOT=str(root / "cache"),
                   PATH=str(bin_dir) + os.pathsep + os.environ.get("PATH", ""))
        env.pop("FZF_DEFAULT_OPTS_FILE", None)
        env.pop("FZF_QUERY", None)

        picker = Picker(binary, env, ["__select"])
        try:
            picker.wait_for(lambda text: "ROOT_ONE_CANARY" in text, "Initial sessions missing")
            assert "Agent: AuditAgent" not in "\n".join(picker.screen.display)
            picker.child.send("\x1b[B")
            picker.child.send("\x01")
            picker.wait_for(lambda text: "Agent: AuditAgent" in text, "Agent expansion failed")
            picker.child.send("title:SECOND_PROJECT_CANARY")
            picker.wait_for(lambda text: "SECOND_PROJECT_CANARY" in text and "ROOT_ONE_CANARY" not in text,
                            "Query reload did not replace the initial list")
            picker.child.send("\x15")
            picker.wait_for(lambda text: "ROOT_ONE_CANARY" in text, "Clearing the query did not restore sessions")
            picker.finish("\x1b")
        finally:
            picker.close()
        print("PASS: empty-query startup, agent expansion, live search, query clearing, cancellation")

        picker = Picker(binary, env, ["__select", "title:SECOND_PROJECT_CANARY"])
        try:
            picker.wait_for(lambda text: "SECOND_PROJECT_CANARY" in text, "Prefilled query did not load sessions")
            assert "ROOT_ONE_CANARY" not in "\n".join(picker.screen.display)
            picker.finish("\r", expected_selection="/demo/beta\tsecond-session")
        finally:
            picker.close()
        print("PASS: prefilled-query startup and workspace selection")

        picker = Picker(binary, env, ["__select"])
        try:
            picker.wait_for(lambda text: "ROOT_ONE_CANARY" in text, "Initial sessions missing")
            # Skip the first workspace's two visible rows and the second's two.
            picker.child.send("\x1b[B" * 4)
            picker.child.send("\x01")
            picker.wait_for(lambda text: "SCAN_ROOT_CANARY" in text, "Scan expansion failed")
            picker.finish("\x1b")
        finally:
            picker.close()
        print("PASS: collapsed security-scan expansion and cancellation")


if __name__ == "__main__":
    main()
