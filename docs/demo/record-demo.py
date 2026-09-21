#!/usr/bin/env python3
"""Foglio README demo choreography.

Drives a real foglio-desktop window (via tauri-driver/WebDriver) against a
disposable demo library, with visible click ripples and human-speed typing,
so the result can be screen-recorded for the README.

The flow, in order:
  1. sandboxed XDG dirs + demo library (fictional notes, including a few
     "car*" titles so the search visibly narrows)
  2. launch app -> switch to light theme
  3. open a note, type a search, open the result
  4. create "Markdown Demo" and type its body in Source mode (fast)
  5. flip to Preview, hold, switch to dark theme, hold

Requires: tauri-driver + WebKitWebDriver (see tests/acceptance), the desktop
binary built with `npm run tauri -- build --debug --no-bundle`.

Environment overrides:
  FOGLIO_DEMO_BINARY  desktop binary      (default <repo>/target/debug/foglio-desktop)
  FOGLIO_DEMO_HOME    sandbox root        (default ~/.cache/foglio-demo)
  TAURI_DRIVER        tauri-driver path   (default "tauri-driver")

Start the screen recorder BEFORE running this (the script begins its scenes
immediately); see docs/demo/recording.md for the full procedure.
"""
import json
import os
import socket
import subprocess
import sys
import time
import urllib.error
import urllib.request

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
BINARY = os.environ.get("FOGLIO_DEMO_BINARY", f"{REPO}/target/debug/foglio-desktop")
SANDBOX = os.path.expanduser(os.environ.get("FOGLIO_DEMO_HOME", "~/.cache/foglio-demo"))
DRIVER = os.environ.get("TAURI_DRIVER", "tauri-driver")

CONFIG = f"{SANDBOX}/config"
CACHE = f"{SANDBOX}/cache"
LIBRARY = f"{SANDBOX}/library"

env = {k: os.environ[k] for k in ("PATH", "LANG", "LD_LIBRARY_PATH", "DISPLAY",
                                  "WAYLAND_DISPLAY", "XDG_RUNTIME_DIR",
                                  "DBUS_SESSION_BUS_ADDRESS") if k in os.environ}
# WebKitGTK under WebDriver renders unreliably with the DMABUF renderer.
env["WEBKIT_DISABLE_DMABUF_RENDERER"] = "1"
env["XDG_CONFIG_HOME"] = CONFIG
env["XDG_CACHE_HOME"] = CACHE

DEMO_NOTES = {
    "Projects/aurora-launch.md": """---
title: Aurora Launch Plan
tags: [planning]
---

# Aurora Launch Plan

## Milestones
- [x] Naming and identity
- [x] Beta waitlist page
- [ ] Onboarding polish
- [ ] Public launch

## Notes
Waitlist conversion is holding at 41%. Focus next week on the
first-run experience: shorten the form, add a sample notebook.

> Decide by Friday whether the changelog page ships with launch.
""",
    "Projects/standup.md": """# Standup — Monday

- Shipped the search refresh fix
- Reviewing the index migration PR
- Demo recording for the README this week

Next: triage the watcher backlog before the release cut.
""",
    "Journal/2026-09-21.md": """# 2026-09-21

Slow morning, good coffee. The new editor feels right — typing is
snappy even in long notes. Idea: keyboard shortcut to jump between
folders without leaving the keyboard.
""",
    "Recipes/cardamom-buns.md": """# Cardamom Buns

 dough: 500g flour, 240ml milk, 75g butter, 50g sugar, 7g yeast
 filling: butter, sugar, two tablespoons cardamom

1. Knead, rise one hour.
2. Roll, spread, twist into knots.
3. Bake 12 minutes at 225°C.

Verdict: better the second day, if they survive that long.
""",
    # "car*" titles so typing "cardamom" visibly narrows the search results
    "Projects/Carbon Notes.md": "# Carbon Notes\n\nTrack the footprint of the release build.\n",
    "Journal/Career.md": "# Career\n\nReview the roadmap after the launch.\n",
}

STRAY_NOTES = ("Fresh Thoughts.md", "Fresh-Thoughts.md",
               "Markdown Demo.md", "Markdown-Demo.md")

MARKDOWN_BODY = """
Write plain **Markdown** — Foglio renders the *rest*.

- Local files stay authoritative
- The index is disposable
- [x] Type in Source mode
- [ ] Switch to Preview

> Local-first, always.
"""

# Golden ring drawn around the element about to be clicked, visible on camera.
RIPPLE = """
const el=ELEMENT;
const r=el.getBoundingClientRect();
const d=document.createElement('div');
d.style.cssText=`position:fixed;left:${r.left+r.width/2-34}px;top:${r.top+r.height/2-34}px;width:68px;height:68px;border:5px solid #ffcc33;border-radius:50%;z-index:99999;pointer-events:none;box-shadow:0 0 18px #ffcc33;transform:scale(.25);opacity:1;transition:transform .45s ease-out,opacity .45s ease-out`;
document.body.appendChild(d);
requestAnimationFrame(()=>requestAnimationFrame(()=>{d.style.transform='scale(1.5)';d.style.opacity='0';}));
setTimeout(()=>d.remove(),700);
"""


def port():
    sock = socket.socket()
    sock.bind(("127.0.0.1", 0))
    picked = sock.getsockname()[1]
    sock.close()
    return picked


class Driver:
    def __init__(self, base):
        self.base, self.session = base, None

    def request(self, method, path, body=None):
        req = urllib.request.Request(self.base + path, method=method,
            data=None if body is None else json.dumps(body).encode(),
            headers={"Content-Type": "application/json"})
        try:
            with urllib.request.urlopen(req, timeout=60) as resp:
                return json.load(resp).get("value")
        except urllib.error.HTTPError as err:
            raise AssertionError(err.read().decode())

    def start(self):
        value = self.request("POST", "/session", {"capabilities": {"alwaysMatch": {
            "tauri:options": {"application": BINARY}}}})
        self.session = value["sessionId"]

    def js(self, script, *args):
        return self.request("POST", f"/session/{self.session}/execute/sync",
                            {"script": script, "args": list(args)})

    def js_async(self, script, *args):
        return self.request("POST", f"/session/{self.session}/execute/async",
                            {"script": script, "args": list(args)})

    def click(self, selector):
        """Ripple at the element centre (visible on camera), then click it."""
        self.js(RIPPLE.replace("ELEMENT", f"document.querySelector({json.dumps(selector)})"))
        time.sleep(0.55)
        self.js("document.querySelector(arguments[0]).click()", selector)

    def click_text(self, text):
        """Ripple + click the first clickable element whose text contains `text`."""
        find = ("[...document.querySelectorAll('button,[role=button],li,a')]"
                f".find(e=>e.textContent.includes({json.dumps(text)}))")
        self.js(RIPPLE.replace("ELEMENT", f"({find})"))
        time.sleep(0.55)
        self.js(find + ".click()")

    def fill(self, selector, value):
        el = self.request("POST", f"/session/{self.session}/element",
                          {"using": "css selector", "value": selector})
        key = el["element-6066-11e4-a52e-4f735466cecf"]
        self.request("POST", f"/session/{self.session}/element/{key}/clear", {})
        # WebKitWebDriver rejects an empty "text" parameter, and a bare clear
        # does not fire the input event the app listens to.
        self.js("document.querySelector(arguments[0])"
                ".dispatchEvent(new Event('input',{bubbles:true}))", selector)
        if value:
            self.request("POST", f"/session/{self.session}/element/{key}/value",
                         {"text": value})

    def type_slow(self, selector, text, ms=45, append=False):
        """Type character by character inside the page so the camera sees it.

        append=True keeps the existing content (e.g. the auto-created
        '# <title>' heading of a new note) and types after it.
        """
        script = """
const t=document.querySelector(arguments[0]);
const text=arguments[1], ms=arguments[2], append=arguments[3];
const done=arguments[arguments.length-1];
t.focus();
let i=0;
if(!append){ t.value=''; }
t.setSelectionRange(t.value.length, t.value.length);
function step(){
  if(i>=text.length){ t.dispatchEvent(new Event('input',{bubbles:true})); done(true); return; }
  const ch=text[i++];
  t.setRangeText(ch, t.selectionStart, t.selectionEnd, 'end');
  t.dispatchEvent(new Event('input',{bubbles:true}));
  setTimeout(step, ms + Math.floor(Math.random()*40));
}
step();"""
        self.js_async(script, selector, text, ms, append)

    def close(self):
        if self.session:
            self.request("DELETE", f"/session/{self.session}")
            self.session = None


def wait(fn, description, timeout=20):
    deadline = time.monotonic() + timeout
    last = None
    while time.monotonic() < deadline:
        try:
            if fn():
                return
        except (AssertionError, OSError) as error:
            last = error
        time.sleep(0.05)
    raise AssertionError(f"timed out: {description}; {last}")


def theme(driver, value):
    """Switch appearance through the in-app dialog."""
    driver.click("[data-testid=appearance-settings]")
    wait(lambda: driver.js(
        "return !!document.querySelector('[data-testid=appearance-preference]')"),
        "appearance dialog")
    driver.js("""const s=document.querySelector('[data-testid=appearance-preference]');
      s.value=arguments[0];
      s.dispatchEvent(new Event('change',{bubbles:true}));
      s.dispatchEvent(new Event('input',{bubbles:true}));""", value)
    time.sleep(0.6)
    driver.click("[data-testid=dialog-submit]")
    wait(lambda: driver.js(
        f"return document.documentElement.dataset.appearance==='{value}'"),
        f"appearance={value}")


def main():
    for binary in (BINARY,):
        if not os.path.isfile(binary):
            sys.exit(f"missing desktop binary: {binary} "
                     "(build with `npm run tauri -- build --debug --no-bundle` in apps/desktop)")

    # Disposable demo library: fresh state on every run.
    subprocess.run(["rm", "-rf", LIBRARY], check=True)
    for rel, content in DEMO_NOTES.items():
        path = f"{LIBRARY}/{rel}"
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, "w") as handle:
            handle.write(content)
    os.makedirs(f"{CONFIG}/foglio", exist_ok=True)
    os.makedirs(CACHE, exist_ok=True)
    with open(f"{CONFIG}/foglio/config.json", "w") as handle:
        json.dump(LIBRARY, handle)

    driver_port, native_port = port(), port()
    driver = Driver(f"http://127.0.0.1:{driver_port}")
    log = open(f"{SANDBOX}/driver.log", "w+")
    proc = subprocess.Popen(
        [DRIVER, "--port", str(driver_port), "--native-port", str(native_port)],
        env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
    try:
        wait(lambda: driver.request("GET", "/status"), "driver ready")
        time.sleep(4)
        print("SESSION: creating app window", flush=True)
        driver.start()
        wait(lambda: driver.js(
            "return document.body.textContent.includes('Aurora Launch Plan')"),
            "notes loaded")
        print("SCENE: library loaded", flush=True)
        time.sleep(2)
        theme(driver, "light")
        print("SCENE: light theme", flush=True)
        time.sleep(1)

        # open a note with a ripple
        driver.click_text("Aurora Launch Plan")
        wait(lambda: driver.js(
            "return document.body.textContent.includes('Milestones')"), "aurora open")
        print("SCENE: note opened", flush=True)
        time.sleep(2.5)

        # search with visible typing; "car" matches three notes, then narrows
        driver.click("[data-testid=search-input]")
        driver.type_slow("[data-testid=search-input]", "cardamom", 70)
        time.sleep(1.5)
        print("SCENE: searching", flush=True)
        driver.click_text("Cardamom Buns")
        wait(lambda: driver.js(
            "return document.body.textContent.includes('Knead, rise one hour')"),
            "cardamom open")
        print("SCENE: search result opened", flush=True)
        time.sleep(2)
        driver.click("[data-testid=search-input]")
        driver.fill("[data-testid=search-input]", "")
        time.sleep(1)

        # new note: the app pre-fills the '# <title>' heading
        driver.click("[data-testid=new-note]")
        wait(lambda: driver.js(
            "return !!document.querySelector('[data-testid=operation-value]')"),
            "new-note dialog")
        driver.type_slow("[data-testid=operation-value]", "Markdown Demo", 55)
        time.sleep(0.8)
        driver.click("[data-testid=dialog-submit]")
        wait(lambda: driver.js("""
          const t=document.querySelector('textarea[data-testid=source]');
          return !!t && t.offsetParent !== null;"""), "editor visible")
        print("SCENE: new note created", flush=True)
        time.sleep(1)

        # body typed fast after the auto-created heading, then Preview
        driver.type_slow("textarea[data-testid=source]", MARKDOWN_BODY, 4, append=True)
        time.sleep(1.5)
        print("SCENE: markdown typed", flush=True)
        driver.click("[data-testid=preview-mode]")
        wait(lambda: driver.js("return !!document.querySelector('h1, h2')"),
             "preview rendered")
        print("SCENE: preview rendered", flush=True)
        time.sleep(3)
        theme(driver, "dark")
        print("SCENE: dark theme", flush=True)
        time.sleep(5)
        print("DONE", flush=True)
    finally:
        driver.close()
        proc.terminate()


if __name__ == "__main__":
    main()
