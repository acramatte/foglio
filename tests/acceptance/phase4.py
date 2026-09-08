#!/usr/bin/env python3
"""Real Linux Tauri/WebKit acceptance. No mocked IPC or browser-only substitute.

Build with `npm run tauri -- build --debug --no-bundle` in apps/desktop.
Requires tauri-driver 2.0.6, WebKitWebDriver, xvfb-run and Python's stdlib.
"""
import http.server
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import tempfile
import threading
import time
import urllib.error
import urllib.request

ROOT = Path(__file__).resolve().parents[2]
BINARY = Path(os.environ.get("FOGLIO_DESKTOP_BINARY", ROOT / "target/debug/foglio-desktop"))
DRIVER = os.environ.get("TAURI_DRIVER", "tauri-driver")
ID = "01ARZ3NDEKTSV4RRFFQ69G5FAV"
OTHER_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAW"


def port():
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


class Driver:
    def __init__(self, base):
        self.base = base
        self.session = None

    def request(self, method, path, body=None):
        request = urllib.request.Request(self.base + path, method=method,
            data=None if body is None else json.dumps(body).encode(),
            headers={"Content-Type": "application/json"})
        try:
            with urllib.request.urlopen(request, timeout=30) as response:
                value = json.load(response).get("value")
        except urllib.error.HTTPError as error:
            raise AssertionError(error.read().decode()) from error
        if isinstance(value, dict) and value.get("error") and "ok" not in value:
            raise AssertionError(value)
        return value

    def start(self):
        value = self.request("POST", "/session", {"capabilities": {"alwaysMatch": {
            "tauri:options": {"application": str(BINARY.resolve())}}}})
        self.session = value["sessionId"]

    def js(self, script, *args):
        return self.request("POST", f"/session/{self.session}/execute/sync", {"script": script, "args": list(args)})

    def invoke(self, command, args=None):
        return self.request("POST", f"/session/{self.session}/execute/async", {
            "script": "const done=arguments[arguments.length-1]; window.__TAURI_INTERNALS__.invoke(arguments[0],arguments[1]).then(v=>done({ok:true,value:v}),e=>done({ok:false,error:e}));",
            "args": [command, args or {}]})

    def close(self):
        if self.session:
            self.request("DELETE", f"/session/{self.session}")
            self.session = None

    def fill(self, selector, value):
        element = self.request("POST", f"/session/{self.session}/element", {"using": "css selector", "value": selector})
        key = element["element-6066-11e4-a52e-4f735466cecf"]
        self.request("POST", f"/session/{self.session}/element/{key}/clear", {})
        if value:
            self.request("POST", f"/session/{self.session}/element/{key}/value", {"text": value})
        else:
            self.js("document.querySelector(arguments[0]).dispatchEvent(new Event('input', {bubbles:true}))", selector)


def wait(test, description, timeout=15):
    deadline = time.monotonic() + timeout
    last = None
    while time.monotonic() < deadline:
        try:
            value = test()
            if value:
                return value
        except (AssertionError, OSError) as error:
            last = error
        time.sleep(0.05)
    raise AssertionError(f"Timed out: {description}; last error: {last}")


def main():
    assert BINARY.is_file(), f"Build the actual desktop binary first: {BINARY}"
    assert shutil.which(DRIVER), f"Missing tauri-driver: {DRIVER}"
    assert shutil.which("xvfb-run") and shutil.which("WebKitWebDriver")
    requests = []

    class Trap(http.server.BaseHTTPRequestHandler):
        def do_GET(self):
            requests.append(self.path)
            self.send_response(200)
            self.end_headers()
        def log_message(self, format, *args):
            pass

    trap = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Trap)
    threading.Thread(target=trap.serve_forever, daemon=True).start()
    with tempfile.TemporaryDirectory(prefix="foglio-phase4-") as directory:
        sandbox = Path(directory)
        library = sandbox / "notes"
        (library / "Projects").mkdir(parents=True)
        (library / "Empty").mkdir()
        note = library / "Projects/first.md"
        note.write_text(f'''---
id: {ID}
tags: [work]
---
# Native preview

| Column | Value |
| --- | --- |
| Safe | Table |

- [x] Task

~~Strike~~ and `code`.

[Second](../second.md)

![Blocked image](http://127.0.0.1:{trap.server_port}/image)

<script>window.foglioInjected=true</script>
<img src="http://127.0.0.1:{trap.server_port}/raw" onerror="window.foglioInjected=true">
<iframe src="https://example.invalid"></iframe>
[Bad](javascript:window.foglioInjected=true)
[Outside](../../outside.md)
''')
        (library / "second.md").write_text(f"---\nid: {OTHER_ID}\n---\n# Second note\nUniqueSearchTerm\n")
        (library / "unadopted.md").write_text("# Imported without ID\n")
        (sandbox / "outside.md").write_text("OUTSIDE MUST NOT BE READ")
        before = {str(p.relative_to(library)): p.read_bytes() for p in library.rglob("*.md")}
        env = {key: os.environ[key] for key in ("PATH", "LANG", "LD_LIBRARY_PATH") if key in os.environ}
        for key, folder in (("HOME", "home"), ("XDG_CONFIG_HOME", "config"), ("XDG_CACHE_HOME", "cache"), ("XDG_DATA_HOME", "data"), ("XDG_RUNTIME_DIR", "runtime")):
            path = sandbox / folder
            path.mkdir(mode=0o700)
            env[key] = str(path)
        env["WEBKIT_DISABLE_DMABUF_RENDERER"] = "1"
        driver_port, native_port = port(), port()
        driver = Driver(f"http://127.0.0.1:{driver_port}")
        with (sandbox / "driver.log").open("w+") as log:
            process = subprocess.Popen(["xvfb-run", "-a", DRIVER, "--port", str(driver_port), "--native-port", str(native_port)], env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
            try:
                wait(lambda: driver.request("GET", "/status"), "native driver ready")
                driver.start()
                wait(lambda: driver.js("return !!document.querySelector('[data-testid=root-path]')"), "onboarding")
                driver.fill("[data-testid=root-path]", str(library))
                driver.js("document.querySelector('[data-testid=select-library]').click()")
                wait(lambda: driver.js("return document.body.textContent.includes('Native preview')"), "notes loaded")
                assert before == {str(p.relative_to(library)): p.read_bytes() for p in library.rglob("*.md")}, "selection adopted/modified notes"
                state = driver.invoke("desktop_state")["value"]
                session = state["session"]
                assert state["root"] == str(library) and state["watcher_active"], state
                browse = driver.invoke("browse_library", {"session": session})
                assert browse["ok"], browse
                assert "Empty" in browse["value"]["folders"], "physical empty folder missing"
                driver.js("Array.from(document.querySelectorAll('[data-testid=note-list] button')).find(e=>e.textContent.includes('Native preview')).click()")
                wait(lambda: driver.js("return !!document.querySelector('[data-testid=preview] table')"), "GFM table in actual webview")
                assert driver.js("return !window.foglioInjected && !document.querySelector('[data-testid=preview] img, [data-testid=preview] script, [data-testid=preview] iframe')"), "unsafe preview DOM"
                assert not requests, f"preview made remote requests: {requests}"
                driver.js("Array.from(document.querySelectorAll('[data-testid=preview] a')).find(e=>e.textContent==='Second').click()")
                wait(lambda: driver.js("return document.querySelector('[data-testid=preview]').textContent.includes('UniqueSearchTerm')"), "contained parent note link")
                driver.js("Array.from(document.querySelectorAll('nav button')).find(e=>e.textContent==='# work').click()")
                wait(lambda: driver.js("return !document.querySelector('[data-testid=note-list]').textContent.includes('Second note')"), "tag filter")
                driver.js("Array.from(document.querySelectorAll('nav button')).find(e=>e.textContent==='All notes').click()")
                driver.js("Array.from(document.querySelectorAll('nav button')).find(e=>e.textContent==='Empty').click()")
                wait(lambda: driver.js("return document.querySelector('[data-testid=note-list]').textContent.includes('No matching notes')"), "empty physical folder filter")
                driver.js("Array.from(document.querySelectorAll('nav button')).find(e=>e.textContent==='All notes').click()")
                driver.js("document.querySelector('[data-note-id=\"'+arguments[0]+'\"]').click()", ID)
                wait(lambda: driver.js("return !!document.querySelector('[data-testid=preview] table')"), "return to original note")
                assert not driver.invoke("resolve_note_link", {"session": session, "fromPath": "Projects/first.md", "target": "../../outside.md"})["ok"]
                assert not driver.invoke("open_external_link", {"url": "javascript:alert(1)"})["ok"]
                assert not driver.invoke("open_note", {"session": session, "id": "../outside.md"})["ok"]
                driver.fill("[data-testid=search-input]", "UniqueSearchTerm")
                wait(lambda: driver.js("return document.querySelector('[data-testid=note-list]').textContent.includes('Second note') && !document.querySelector('[data-testid=note-list]').textContent.includes('Native preview')"), "native FTS search")
                driver.fill("[data-testid=search-input]", "")
                wait(lambda: driver.js("return document.querySelector('[data-testid=note-list]').textContent.includes('Native preview')"), "clear search")
                subprocess.run(["python3", "-c", "import pathlib,sys;p=pathlib.Path(sys.argv[1]);p.write_text(p.read_text().replace('Native preview','External refresh'));p.rename(p.with_name('renamed.md'))", str(note)], check=True)
                wait(lambda: driver.js("return document.querySelector('[data-testid=preview]').textContent.includes('External refresh')"), "watcher refresh after external move/edit")
                current = driver.invoke("open_note", {"session": session, "id": ID})
                assert current["ok"] and current["value"]["path"] == "Projects/renamed.md", current
                renamed = library / "Projects/renamed.md"
                copy = library / "conflict.md"
                shutil.copyfile(renamed, copy)
                wait(lambda: driver.js("return document.querySelector('[data-testid=preview]').textContent.includes('Note unavailable') && document.querySelector('[data-testid=diagnostics]').textContent.toLowerCase().includes('ambiguous')"), "duplicate conflict surfaced")
                assert copy.read_bytes() == renamed.read_bytes()
                copy.unlink()
                wait(lambda: driver.js("return document.querySelector('[data-testid=preview]').textContent.includes('External refresh')"), "duplicate recovery")
                renamed.chmod(0)
                try:
                    wait(lambda: driver.js("return document.querySelector('[data-testid=preview]').textContent.includes('Note unavailable')"), "unreadable note surfaced")
                finally:
                    renamed.chmod(0o600)
                wait(lambda: driver.js("return document.querySelector('[data-testid=preview]').textContent.includes('External refresh')"), "permission recovery")
                renamed.unlink()
                wait(lambda: driver.js("return !document.querySelector('[data-testid=preview]').textContent.includes('External refresh')"), "deleted selection is cleared")
                driver.close()
                driver.start()
                wait(lambda: driver.js("return document.body.textContent.includes('Second note')"), "persisted root reopened")
                assert (library / "unadopted.md").read_bytes() == before["unadopted.md"]
                assert not requests, requests
                driver.close()
                print("PASS: actual Tauri onboarding, readonly selection, folders, GFM/security, search, watcher move/edit/delete, close/reopen")
            except Exception:
                log.flush()
                log.seek(0)
                print(log.read())
                raise
            finally:
                try:
                    driver.close()
                finally:
                    import signal
                    if process.poll() is None:
                        os.killpg(process.pid, signal.SIGTERM)
                    process.wait(timeout=10)
    trap.shutdown()


if __name__ == "__main__":
    main()
