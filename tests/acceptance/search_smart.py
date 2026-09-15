#!/usr/bin/env python3
"""Native acceptance for Smart search: `memo` must reach a note titled `Memory`.

Real Linux Tauri/WebKit. No mocked IPC, no browser-only substitute: the desktop
backend, the FTS index and the rendered note list are all exercised.

Build with `npm run tauri -- build --debug --no-bundle` in apps/desktop.
Requires tauri-driver 2.0.6, WebKitWebDriver, xvfb-run and Python's stdlib.
"""
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import tempfile
import time
import urllib.error
import urllib.request

ROOT = Path(__file__).resolve().parents[2]
BINARY = Path(os.environ.get("FOGLIO_DESKTOP_BINARY", ROOT / "target/debug/foglio-desktop"))
DRIVER = os.environ.get("TAURI_DRIVER", "tauri-driver")


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
    with tempfile.TemporaryDirectory(prefix="foglio-smart-search-") as directory:
        sandbox = Path(directory)
        library = sandbox / "notes"
        library.mkdir(parents=True)
        # Only the title carries the complete word for `memo`.
        (library / "memory.md").write_text("# Memory\nKeep retros short\n")
        # Only the body carries it, and the title must not match.
        (library / "spending.md").write_text("# Spending\nbudget report and memory notes\n")
        (library / "shopping.md").write_text("# Shopping\nbuy milk\n")
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
                wait(lambda: driver.js("return document.querySelectorAll('.note-card').length===3"), "notes loaded")
                # A partial word must list the note whose title completes it.
                driver.fill("[data-testid=search-input]", "memo")
                wait(lambda: driver.js("return document.querySelectorAll('.note-card').length===2"), "smart search lists complete and partial matches")
                assert driver.js("return document.querySelector('.note-card').dataset.notePath==='memory.md'"), "title match must rank first"
                assert driver.js("return document.querySelector('[data-testid=note-list]').textContent.includes('Memory') && document.querySelector('[data-testid=note-list]').textContent.includes('Spending')"), "both matches listed"
                assert driver.js("return !document.querySelector('[data-testid=note-list]').textContent.includes('Shopping')"), "unrelated note must stay out"
                assert driver.js("return document.querySelector('.note-card .snippet').textContent.length>0"), "results carry snippets"
                driver.js("document.querySelector('.note-card').click()")
                wait(lambda: driver.js("return document.querySelector('.metadata').textContent.includes('memory.md')"), "result opens the matched note")
                assert not driver.js("return document.querySelector('[data-testid=preview]').textContent.includes('Note unavailable')")
                # Interior substrings are still not matches.
                driver.fill("[data-testid=search-input]", "get")
                wait(lambda: driver.js("return document.querySelector('[data-testid=note-list]').textContent.includes('No matching notes')"), "interior substring must not match")
                # Two characters already expand, and clearing restores browsing.
                driver.fill("[data-testid=search-input]", "mem")
                wait(lambda: driver.js("return document.querySelectorAll('.note-card').length===2"), "two-character prefix expands")
                driver.fill("[data-testid=search-input]", "")
                wait(lambda: driver.js("return document.querySelectorAll('.note-card').length===3"), "clearing search restores browsing")
                state = driver.invoke("desktop_state")["value"]
                assert state["root"] == str(library) and state["watcher_active"], state
                driver.close()
                assert before == {str(p.relative_to(library)): p.read_bytes() for p in library.rglob("*.md")}, "searching modified notes"
                print("PASS: actual Tauri smart search surfaces a title-only `Memory` for `memo`, ranks it first, opens it, and rejects interior substrings")
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


if __name__ == "__main__":
    main()