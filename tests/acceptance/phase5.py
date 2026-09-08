#!/usr/bin/env python3
"""Phase 5 native WebKit acceptance; isolated real files, no mock IPC."""
import ctypes
import os
from pathlib import Path
import shutil
import signal
import subprocess
import tempfile
from phase4 import BINARY, DRIVER, Driver, port, wait


def native_close(pid):
    """Send the window manager's real WM_DELETE_WINDOW message to GTK/X11."""
    entries = Path(f"/proc/{pid}/environ").read_bytes().split(b"\0")
    display = next(item.split(b"=", 1)[1].decode() for item in entries if item.startswith(b"DISPLAY="))
    os.environ['XAUTHORITY'] = next(item.split(b"=", 1)[1].decode() for item in entries if item.startswith(b"XAUTHORITY="))
    x = ctypes.CDLL("libX11.so.6")
    x.XOpenDisplay.argtypes = [ctypes.c_char_p]; x.XOpenDisplay.restype = ctypes.c_void_p
    d = x.XOpenDisplay(display.encode()); assert d
    x.XDefaultRootWindow.argtypes = [ctypes.c_void_p]; x.XDefaultRootWindow.restype = ctypes.c_ulong
    x.XInternAtom.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_int]; x.XInternAtom.restype = ctypes.c_ulong
    x.XQueryTree.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.POINTER(ctypes.c_ulong), ctypes.POINTER(ctypes.c_ulong), ctypes.POINTER(ctypes.POINTER(ctypes.c_ulong)), ctypes.POINTER(ctypes.c_uint)]
    x.XFetchName.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.POINTER(ctypes.c_char_p)]
    x.XFree.argtypes = [ctypes.c_void_p]
    x.XSendEvent.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_int, ctypes.c_long, ctypes.c_void_p]
    x.XFlush.argtypes = [ctypes.c_void_p]; x.XCloseDisplay.argtypes = [ctypes.c_void_p]
    root, parent, children, count = ctypes.c_ulong(), ctypes.c_ulong(), ctypes.POINTER(ctypes.c_ulong)(), ctypes.c_uint()
    assert x.XQueryTree(d, x.XDefaultRootWindow(d), ctypes.byref(root), ctypes.byref(parent), ctypes.byref(children), ctypes.byref(count))
    window = None
    for child in children[:count.value]:
        name = ctypes.c_char_p()
        if x.XFetchName(d, child, ctypes.byref(name)) and name.value:
            if name.value.lower() == b"foglio": window = child
            x.XFree(name)
    x.XFree(children)
    assert window, "Foglio native window not found"
    class Data(ctypes.Union):
        _fields_ = [("l", ctypes.c_long * 5), ("b", ctypes.c_char * 20)]
    class Client(ctypes.Structure):
        _fields_ = [("type", ctypes.c_int), ("serial", ctypes.c_ulong), ("send_event", ctypes.c_int), ("display", ctypes.c_void_p), ("window", ctypes.c_ulong), ("message_type", ctypes.c_ulong), ("format", ctypes.c_int), ("data", Data)]
    class Event(ctypes.Union):
        _fields_ = [("client", Client), ("pad", ctypes.c_long * 24)]
    event = Event(); event.client.type = 33; event.client.display = d; event.client.window = window
    event.client.message_type = x.XInternAtom(d, b"WM_PROTOCOLS", 0); event.client.format = 32
    event.client.data.l[0] = x.XInternAtom(d, b"WM_DELETE_WINDOW", 0)
    assert x.XSendEvent(d, window, False, 0, ctypes.byref(event))
    x.XFlush(d); x.XCloseDisplay(d)


def app_pid(home):
    for p in Path('/proc').iterdir():
        if not p.name.isdigit(): continue
        try:
            if p.joinpath('exe').resolve() == BINARY.resolve() and f'HOME={home}'.encode() in p.joinpath('environ').read_bytes().split(b'\0'):
                return int(p.name)
        except (OSError, PermissionError): pass
    raise AssertionError('isolated native app process not found')


def main():
    assert BINARY.is_file() and shutil.which(DRIVER)
    scenarios = []
    with tempfile.TemporaryDirectory(prefix='foglio-phase5-') as directory:
        sandbox = Path(directory); library = sandbox/'notes'; library.mkdir()
        header = b'\xef\xbb\xbf---\r\n# keep comment\r\nid: [arbitrary, metadata]\r\ncustom:\r\n  nested: 42\r\ntags: [work]\r\n---\r\n'
        body = '# Preservation\r\n\r\n| A | B |\r\n|---|---|\r\n| 1 | 2 |\r\n- [x] task\r\n~~strike~~ `code` [link](other.md)\r\n[[wiki]] ![image](https://example.invalid/a.png)\r\n:::custom\r\nuntouched\r\n::: \r\n```unknown\r\n---\r\n```\r\n'
        original = header + body.encode(); note = library/'preserve.md'; note.write_bytes(original)
        (library/'other.md').write_text('# Other\n')
        (library/'stale.md').write_text('# Stale\n')
        (library/'missing.md').write_text('# Missing\n')
        (library/'mixed.md').write_bytes(b'A\r\nB\nC\r\nD')
        (library/'long.md').write_text('# Long note\n\n' + 'A paragraph that makes the preview scroll.\n\n' * 80)
        env = {key:os.environ[key] for key in ('PATH','LANG','LD_LIBRARY_PATH') if key in os.environ}
        for key, folder in [('HOME','home'),('XDG_CONFIG_HOME','config'),('XDG_CACHE_HOME','cache'),('XDG_DATA_HOME','data'),('XDG_RUNTIME_DIR','runtime')]:
            p=sandbox/folder;p.mkdir(mode=0o700);env[key]=str(p)
        env['WEBKIT_DISABLE_DMABUF_RENDERER']='1'
        dport,nport=port(),port();driver=Driver(f'http://127.0.0.1:{dport}')
        def js(script,*args): return driver.js(script,*args)
        def click(selector): js('document.querySelector(arguments[0]).click()',selector)
        def saved(): return js("return document.querySelector('[data-testid=save-status]').dataset.state === 'clean'")
        def source():
            value = js("return document.querySelector('[data-testid=source]').value")
            assert isinstance(value, str)
            return value
        def edit(text): js("const e=document.querySelector('[data-testid=source]');e.value=arguments[0];e.dispatchEvent(new Event('input',{bubbles:true}));",text)
        def open_note(path):
            wait(lambda:js('return !!document.querySelector(arguments[0])',f'[data-note-path="{path}"]'),'note listed '+path)
            click(f'[data-note-path="{path}"]')
            wait(lambda:js('return document.querySelector(".metadata").textContent.includes(arguments[0])',path),'note opened '+path)
        def operation(action,value=None,folder=None,cancel=False):
            click(f'[data-testid={action}]');wait(lambda:js("return !!document.querySelector('dialog[open]')"),'operation dialog')
            if value is not None: driver.fill('[data-testid=operation-value]',value)
            if folder is not None: driver.fill('[data-testid=operation-folder]',folder)
            click('[data-testid=dialog-cancel]' if cancel else '[data-testid=dialog-submit]')
            wait(lambda:js("return !document.querySelector('dialog')"),'dialog closed')
        with (sandbox/'driver.log').open('w+') as log:
            process=subprocess.Popen(['xvfb-run','-a',DRIVER,'--port',str(dport),'--native-port',str(nport)],env=env,stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
            try:
                wait(lambda:driver.request('GET','/status'),'driver ready');driver.start()
                wait(lambda:js("return !!document.querySelector('[data-testid=root-path]')"),'onboarding')
                driver.fill('[data-testid=root-path]',str(library));click('[data-testid=select-library]');open_note('preserve.md')
                assert note.read_bytes()==original
                # No-op source/preview toggles must not serialize the DOM or textarea normalization.
                click('[data-testid=source-mode]');edit(source());click('[data-testid=preview-mode]')
                assert saved() and note.read_bytes()==original
                assert js("return !!document.querySelector('article table') && !document.querySelector('article img')")
                open_note('long.md')
                assert js("const p=document.querySelector('[data-testid=preview]');return p.scrollHeight>p.clientHeight && document.documentElement.scrollHeight===document.documentElement.clientHeight")
                assert js("const p=document.querySelector('[data-testid=preview]');p.scrollTop=200;return p.scrollTop>0")
                scenarios.append('long preview scroll stays inside the note pane')
                open_note('preserve.md')
                click('[data-testid=source-mode]')
                js("const e=document.querySelector('textarea');e.focus();const i=e.value.indexOf('untouched');e.setSelectionRange(i,i+'untouched'.length)")
                driver.request('POST', f'/session/{driver.session}/actions', {'actions':[{'type':'key','id':'keyboard','actions':[{'type':'keyDown','value':'Z'},{'type':'keyUp','value':'Z'}]}]})
                wait(saved,'autosaved preservation fixture')
                expected=header+body.replace('untouched','Z').encode();assert note.read_bytes()==expected, (repr(note.read_bytes()), repr(expected), repr(source()))
                scenarios.append('source roundtrip, BOM/CRLF/unknown YAML and unsupported syntax')
                # Native keyboard toggle works in both directions, including when preview owns no focus.
                js("document.querySelector('textarea').dispatchEvent(new KeyboardEvent('keydown',{key:'e',ctrlKey:true,bubbles:true}))")
                assert js("return document.querySelector('textarea').hidden && !document.querySelector('article').hidden")
                js("document.dispatchEvent(new KeyboardEvent('keydown',{key:'e',ctrlKey:true,bubbles:true}))")
                assert js("return !document.querySelector('textarea').hidden && document.querySelector('article').hidden")
                js("document.dispatchEvent(new KeyboardEvent('keydown',{key:'e',ctrlKey:true,bubbles:true}))")
                assert js("return document.querySelector('textarea').hidden && !document.querySelector('article').hidden")
                operation('new-note','Sprite','blog');wait(lambda:(library/'blog/Sprite.md').exists(),'created nested file')
                wait(lambda:js("return document.querySelector('.metadata').textContent.includes('blog/Sprite.md')"),'created path selected')
                edit('# Created\nBody from desktop\n');wait(saved,'new note autosave')
                created=library/'blog/Sprite.md';assert created.read_text()=='# Created\nBody from desktop\n'
                operation('tag-note','Project');wait(lambda:'tags: ["Project"]' in created.read_text(),'tag on disk')
                wait(lambda:js("return document.querySelector('.metadata').textContent.includes('#Project')"),'tag selected')
                operation('untag-note');wait(lambda:'Project' not in created.read_text(),'tag removed')
                wait(saved,'tag acknowledgement');before=created.read_bytes()
                operation('move-note','nested/renamed.md');renamed=library/'nested/renamed.md';wait(lambda:renamed.exists() and not created.exists(),'renamed file')
                wait(lambda:js("return document.querySelector('.metadata').textContent.includes('nested/renamed.md')"),'rename path selected')
                assert renamed.read_bytes()==before
                operation('move-note','other.md');wait(lambda:js("return !document.querySelector('[role=alert]').hidden"),'collision error');assert renamed.read_bytes()==before and (library/'other.md').read_text()=='# Other\n'
                operation('delete-note',cancel=True);assert renamed.exists()
                operation('delete-note');wait(lambda:not renamed.exists(),'confirmed permanent delete')
                scenarios.append('create/edit/tag/untag/rename/collision/cancel/confirmed deletion')
                # Navigation saves immediately, even before debounce fires.
                open_note('preserve.md');click('[data-testid=source-mode]');edit(source()+'navigation tail\n');click('[data-note-path="other.md"]')
                wait(lambda:js("return document.querySelector('.metadata').textContent.includes('other.md')"),'navigation flushed')
                assert note.read_bytes().endswith(b'navigation tail\r\n')
                scenarios.append('navigation flush before switching')
                open_note('mixed.md');click('[data-testid=source-mode]')
                edit('XA\nB\nC\nD');edit('XA\nB\nC\nDY');wait(saved,'separated mixed-newline edits')
                assert (library/'mixed.md').read_bytes()==b'XA\r\nB\nC\r\nDY'
                scenarios.append('separated edits preserve untouched mixed newlines')
                # Actual permissions fail safely, retry explicitly after restoring access.
                open_note('preserve.md');note.chmod(0o400);edit(source()+'permission tail\n')
                wait(lambda:js("return document.querySelector('[data-testid=save-status]').dataset.state==='save_error'"),'permission failure')
                assert 'permission tail' in source() and b'permission tail' not in note.read_bytes()
                wait(lambda:js("return !!document.querySelector('[data-note-path=\"other.md\"]')"),'other note relisted before failed navigation')
                click('[data-note-path="other.md"]');wait(lambda:js("return !!document.querySelector('dialog[open]')"),'failed navigation blocked');click('[data-testid=dialog-cancel]')
                pid=app_pid(env['HOME']);native_close(pid);wait(lambda:js("return !!document.querySelector('dialog[open]')"),'native close blocked on failure');click('[data-testid=dialog-cancel]')
                assert 'permission tail' in source();note.chmod(0o600)
                js("Array.from(document.querySelectorAll('button')).find(e=>e.textContent==='Retry save').click()")
                wait(saved,'explicit permission retry');assert b'permission tail' in note.read_bytes()
                scenarios.append('permission failure retains buffer and blocks navigation/native close; explicit retry')
                # Stale writes reject without overwriting the other process or clearing the editor.
                open_note('stale.md');edit('# Local stale buffer\n')
                subprocess.run(['python3','-c','import pathlib,sys;pathlib.Path(sys.argv[1]).write_text("# External winner\\n")',str(library/'stale.md')],check=True)
                wait(lambda:js("return document.querySelector('[data-testid=save-status]').dataset.state==='conflict'"),'stale autosave paused')
                assert source()=='# Local stale buffer\n' and (library/'stale.md').read_text()=='# External winner\n'
                scenarios.append('stale autosave preserves disk and local buffer')
                # Close remains protected; use explicit process termination only to reset the fixture.
                native_close(pid);wait(lambda:js("return !!document.querySelector('dialog[open]')"),'conflicted close blocked');click('[data-testid=dialog-cancel]')
                os.kill(pid,signal.SIGTERM);driver.close()
                driver.start();wait(lambda:js("return !!document.querySelector('[data-note-path=\"missing.md\"]')"),'reopened library');open_note('missing.md');click('[data-testid=source-mode]');edit('# Do not resurrect\n');(library/'missing.md').unlink()
                wait(lambda:js("return document.querySelector('[data-testid=save-status]').dataset.state==='missing_on_disk'"),'deleted autosave paused')
                assert source()=='# Do not resurrect\n' and not (library/'missing.md').exists()
                scenarios.append('missing path never resurrected')
                pid=app_pid(env['HOME']);os.kill(pid,signal.SIGTERM);driver.close()
                driver.start();wait(lambda:js("return !!document.querySelector('[data-note-path=\"other.md\"]')"),'reopened for close');open_note('other.md');click('[data-testid=source-mode]');edit('# Last edit before native close\n')
                pid=app_pid(env['HOME']);native_close(pid)
                wait(lambda:not Path(f'/proc/{pid}').exists(),'native close completed after save')
                assert (library/'other.md').read_text()=='# Last edit before native close\n';driver.session=None
                scenarios.append('native WM_DELETE_WINDOW flushes dirty note then exits')
                for scenario in scenarios: print('PASS: '+scenario)
                print(f'PASS: {len(scenarios)} Phase 5 native scenarios')
            except Exception:
                log.flush();log.seek(0);print(log.read())
                if driver.session:
                    print(js('return document.body.innerText'))
                    print(driver.invoke('desktop_state'))
                    print(js('return document.querySelector("[data-testid=diagnostics]")?.textContent'))
                raise
            finally:
                os.killpg(process.pid,signal.SIGTERM)
                try: process.wait(timeout=10)
                except subprocess.TimeoutExpired: os.killpg(process.pid,signal.SIGKILL);process.wait()


if __name__=='__main__': main()
