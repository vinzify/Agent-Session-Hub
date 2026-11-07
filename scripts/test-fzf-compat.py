"""Exercise real picker startup, search, expansion and cancellation in a PTY.

All session data is fictional and isolated in a temporary directory.
Requires pexpect and pyte; see docs/architecture.md for invocation.
"""

import argparse
import json
import os
from pathlib import Path
import sqlite3
import tempfile
import time

import pexpect
import pyte


class Picker:
    def __init__(self, binary, env, args, provider="codex"):
        self.screen = pyte.Screen(180, 32)
        self.stream = pyte.Stream(self.screen)
        self.transcript = ""
        launch_args = args if provider is None else ["--provider", provider, *args]
        self.child = pexpect.spawn(
            str(binary), launch_args, env=env,
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

    def finish(self, key, expected_selection=None, expected_status=0):
        self.child.send(key)
        deadline = time.monotonic() + 12
        while time.monotonic() < deadline:
            if not self.read():
                break
        else:
            raise AssertionError("Picker did not exit")
        self.child.close()
        assert self.child.exitstatus == expected_status, self.transcript[-2000:]
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
        root = Path(temporary).resolve()
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
                   AGENT_SESSION_HUB_NO_UPDATE_CHECK="1",
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

        # A fourth provider must use its own index in reload/preview helpers,
        # without invoking agy or opening a real conversation during QA.
        antigravity = root / "antigravity"
        (antigravity / "conversations").mkdir(parents=True)
        project = root / "Sicilia café project"
        project.mkdir()
        with sqlite3.connect(antigravity / "conversation_summaries.db") as db:
            db.execute("CREATE TABLE conversation_summaries (conversation_id TEXT, title TEXT, "
                       "preview TEXT, last_modified_time TEXT, workspace_uris TEXT, "
                       "parent_conversation_id TEXT, nesting_depth INTEGER, agent_name TEXT)")
            db.execute("INSERT INTO conversation_summaries VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                       ("ag-chat", "ANTIGRAVITY_CANARY prepare release",
                        "AG_PREVIEW_CANARY review the release plan",
                        "2026-11-07 10:43:48.388633693+00:00", json.dumps([project.as_uri()]), "", 0, ""))
            db.execute("INSERT INTO conversation_summaries VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                       ("ag-agent", "Antigravity agent review", "Check the release",
                        "2026-11-07 10:40:00+00:00", json.dumps([project.as_uri()]), "ag-chat", 1, "AGAuditAgent"))
        for identity in ("ag-chat", "ag-agent"):
            with sqlite3.connect(antigravity / f"conversations/{identity}.db") as db:
                db.execute("CREATE TABLE steps (idx INTEGER PRIMARY KEY, step_payload BLOB)")
        env["CODEX_SESSION_HUB_ANTIGRAVITY_SESSION_ROOT"] = str(antigravity)
        picker = Picker(binary, env, ["__select"], provider="antigravity")
        try:
            picker.wait_for(lambda text: "ANTIGRAVITY_CANARY" in text, "Antigravity startup missing")
            picker.child.send("title:ANTIGRAVITY_CANARY")
            picker.child.send("\x1b[B")
            picker.wait_for(lambda text: "AG_PREVIEW_CANARY" in text, "Antigravity preview missing")
            picker.finish("\r", expected_selection=str(project) + "\tag-chat")
        finally:
            picker.close()
        print("PASS: Antigravity index loading, title search, preview, and workspace selection")

        picker = Picker(binary, env, ["__select"], provider="antigravity")
        try:
            picker.wait_for(lambda text: "ANTIGRAVITY_CANARY" in text, "Antigravity startup missing")
            picker.child.send("\x1b[B\x01")
            picker.wait_for(lambda text: "Agent: AGAuditAgent" in text, "Antigravity expansion failed")
            picker.child.send("title:AGAuditAgent")
            picker.wait_for(lambda text: "ANTIGRAVITY_CANARY" in text and "Agent: AGAuditAgent" in text,
                            "Antigravity agent search lost its parent chat")
            picker.finish("\x1b")
        finally:
            picker.close()
        print("PASS: Antigravity agent expansion, parent-preserving search, and cancellation")

        claude = root / "claude"
        for relative, title in (
            ("claude-root.jsonl", "CLAUDE_ROOT_CANARY plan release"),
            ("claude-root/subagents/agent-review.jsonl", "Check the release"),
        ):
            path = claude / "project" / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(json.dumps({"sessionId": "claude-root", "cwd": str(project),
                                       "timestamp": "2026-11-07T10:00:00Z", "type": "user",
                                       "message": {"content": title}}) + "\n")
        env["CODEX_SESSION_HUB_CLAUDE_SESSION_ROOT"] = str(claude)
        picker = Picker(binary, env, ["__select"], provider="claude")
        try:
            picker.wait_for(lambda text: "CLAUDE_ROOT_CANARY" in text, "Claude startup missing")
            picker.child.send("\x1b[B\x01")
            picker.wait_for(lambda text: "Agent: review" in text, "Claude expansion failed")
            picker.child.send("\x1b[B")
            picker.finish("\r", expected_selection=str(project) + "\tclaude-root")
        finally:
            picker.close()
        print("PASS: Claude agent expansion and resuming the connected root conversation")

        # Exercise the public no-argument menu, not just provider-qualified
        # hidden helpers. Native CLIs are stubs: no real chat/model is opened.
        opencode = root / "opencode"
        opencode.mkdir()
        with sqlite3.connect(opencode / "opencode.db") as db:
            db.executescript(
                "CREATE TABLE workspace (id TEXT, branch TEXT);"
                "CREATE TABLE session (id TEXT PRIMARY KEY, slug TEXT, directory TEXT, title TEXT, "
                "time_created INTEGER, time_updated INTEGER, time_archived INTEGER, workspace_id TEXT, parent_id TEXT);"
                "CREATE TABLE message (id TEXT, session_id TEXT, time_created INTEGER, data TEXT);"
                "CREATE TABLE part (id TEXT, message_id TEXT, time_created INTEGER, data TEXT);"
            )
            db.execute("INSERT INTO session VALUES (?,?,?,?,?,?,NULL,NULL,NULL)",
                       ("op-chat", "op-chat", str(project), "OPENCODE_ROOT_CANARY plan release", 1, 1))
        env["CODEX_SESSION_HUB_OPENCODE_SESSION_ROOT"] = str(opencode)
        for provider, native in (("codex", "codex"), ("claude", "claude"),
                                 ("opencode", "opencode"), ("antigravity", "agy")):
            stub = bin_dir / native
            stub.write_text(
                "#!/bin/sh\n"
                f"printf 'PROVIDER_STUB_CANARY:{provider}\\n'\n"
                "printf 'ARG:%s\\n' \"$@\"\n"
                "printf 'CWD:%s\\n' \"$PWD\"\n"
            )
            stub.chmod(0o755)

        for choice, provider, canary, resume_args in (
            ("1", "codex", "ROOT_ONE_CANARY", ("resume", "--cd", "/demo/alpha", "root-session")),
            ("2", "claude", "CLAUDE_ROOT_CANARY", ("--resume", "claude-root")),
            ("3", "opencode", "OPENCODE_ROOT_CANARY", ("--session", "op-chat")),
            ("4", "antigravity", "ANTIGRAVITY_CANARY", ("--conversation", "ag-chat")),
        ):
            picker = Picker(binary, env, [], provider=None)
            try:
                picker.wait_for(lambda text: "Choice [1-4,q]:" in text, "Provider menu missing")
                if choice == "1":
                    picker.child.send("9\r")
                    picker.wait_for(lambda text: "Unknown choice: 9" in text, "Invalid menu choice not handled")
                picker.child.send(choice + "\r")
                picker.wait_for(lambda text: canary in text, f"Menu did not open the {provider} picker")
                picker.finish("\r", expected_selection=f"PROVIDER_STUB_CANARY:{provider}")
                for argument in resume_args:
                    assert f"ARG:{argument}" in picker.transcript, picker.transcript[-2000:]
                if provider != "codex":
                    assert f"CWD:{project}" in picker.transcript, picker.transcript[-2000:]
            finally:
                picker.close()
        print("PASS: all four menu choices, invalid choice, real picker selection, and correct resume commands")

        for key in ("q\r", "\r", "\x04"):
            picker = Picker(binary, env, [], provider=None)
            try:
                picker.wait_for(lambda text: "Choice [1-4,q]:" in text, "Provider menu missing")
                picker.finish(key)
                assert "PROVIDER_STUB_CANARY:" not in picker.transcript
            finally:
                picker.close()
        print("PASS: menu quit, empty-input cancellation, and EOF without launching a provider")

        picker = Picker(binary, env, ["title:SECOND_PROJECT_CANARY"], provider=None)
        try:
            picker.wait_for(lambda text: "Choice [1-4,q]:" in text, "Query did not prompt for a provider")
            picker.child.send("csx\r")
            picker.wait_for(lambda text: "SECOND_PROJECT_CANARY" in text and "ROOT_ONE_CANARY" not in text,
                            "Menu did not forward the original query")
            picker.finish("\r", expected_selection="PROVIDER_STUB_CANARY:codex")
            assert "ARG:second-session" in picker.transcript
        finally:
            picker.close()
        print("PASS: named menu choice and original-query forwarding")

        picker = Picker(binary, env, ["opencode"], provider=None)
        try:
            picker.wait_for(lambda text: "OPENCODE_ROOT_CANARY" in text, "Explicit provider did not open its picker")
            assert "Select CLI to launch:" not in picker.transcript
            picker.finish("\r", expected_selection="PROVIDER_STUB_CANARY:opencode")
        finally:
            picker.close()
        print("PASS: explicit provider bypasses the menu and resumes the correct conversation")

        # A supported provider can be listed even when its native CLI is absent.
        # Verify a readable failure instead of a crash or the wrong executable.
        (bin_dir / "claude").unlink()
        missing_env = dict(env, PATH=str(bin_dir))
        picker = Picker(binary, missing_env, [], provider=None)
        try:
            picker.wait_for(lambda text: "Choice [1-4,q]:" in text, "Provider menu missing")
            picker.child.send("2\r")
            picker.wait_for(lambda text: "CLAUDE_ROOT_CANARY" in text, "Missing-provider history picker failed")
            picker.finish("\r", expected_selection="claude was not found in PATH", expected_status=1)
        finally:
            picker.close()
        print("PASS: missing native CLI produces a clear error after menu selection")


if __name__ == "__main__":
    main()
