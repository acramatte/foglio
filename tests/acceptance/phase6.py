#!/usr/bin/env python3
"""Phase 6 native WebKit conflicts with separate real filesystem writers.

The saving scenario delays delivery of a real IPC acknowledgement, not the
command or its result. No filesystem/IPC result is mocked.
"""
import os
from pathlib import Path
import shutil
import signal
import subprocess
import tempfile
from phase4 import BINARY, DRIVER, Driver, port, wait
from phase5 import app_pid, native_close


def main():
    assert BINARY.is_file() and shutil.which(DRIVER)
    scenarios = []
    with tempfile.TemporaryDirectory(prefix='foglio-phase6-') as directory:
        sandbox = Path(directory)
        library = sandbox/'notes'
        library.mkdir()
        prefix = b'\xef\xbb\xbf---\r\n# original metadata\r\nid: [user, metadata]\r\ncustom:\r\n  nested: 42\r\ntags: [work]\r\n---\r\n'
        for name in ('clean', 'conflict', 'copy', 'move', 'delete', 'saving', 'saving-close', 'sync'):
            (library/f'{name}.md').write_bytes(prefix + f'# {name}\r\nOriginal\nEnd\r\n'.encode())
        (library/'occupied.md').write_text('# Occupied\n')
        env = {key: os.environ[key] for key in ('PATH', 'LANG', 'LD_LIBRARY_PATH') if key in os.environ}
        for key, folder in [('HOME','home'),('XDG_CONFIG_HOME','config'),('XDG_CACHE_HOME','cache'),('XDG_DATA_HOME','data'),('XDG_RUNTIME_DIR','runtime')]:
            p = sandbox/folder
            p.mkdir(mode=0o700)
            env[key] = str(p)
        env['WEBKIT_DISABLE_DMABUF_RENDERER'] = '1'
        dport, nport = port(), port()
        driver = Driver(f'http://127.0.0.1:{dport}')
        js = driver.js

        def click(testid):
            js('document.querySelector(arguments[0]).click()', f'[data-testid="{testid}"]')

        def state():
            return js("return document.querySelector('[data-testid=save-status]').dataset.state")

        def source():
            return js('return document.querySelector("textarea").value')

        def edit(body):
            js("const e=document.querySelector('textarea');e.value=arguments[0];e.dispatchEvent(new Event('input',{bubbles:true}))", body)

        def external(name, body=None, destination=None):
            # Independent process, byte-exact writes/renames/deletes like an editor/sync tool.
            program = 'import pathlib,sys; p=pathlib.Path(sys.argv[1]); '
            if destination:
                program += 'p.rename(sys.argv[2])'
                args = [str(library/name), str(library/destination)]
            elif body is None:
                program += 'p.unlink()'
                args = [str(library/name)]
            else:
                program += 'p.write_bytes(bytes.fromhex(sys.argv[2]))'
                args = [str(library/name), body.hex()]
            subprocess.run(['python3', '-c', program, *args], check=True)

        def open_note(name):
            selector = f'[data-note-path="{name}"]'
            wait(lambda: js('const e=document.querySelector(arguments[0]);if(!e)return false;e.click();return true', selector), 'listed note clicked '+name)
            wait(lambda: js('return document.querySelector(".metadata").textContent.includes(arguments[0]) && !document.querySelector("textarea").readOnly', name), 'opened '+name)
            click('source-mode')

        def choice(action):
            click('conflict-'+action)
            wait(lambda: js("return !!document.querySelector('dialog[open]')"), 'conflict dialog')

        def finish(path=None, cancel=False):
            if path is not None:
                driver.fill('[data-testid=operation-value]', path)
            click('dialog-cancel' if cancel else 'dialog-submit')
            wait(lambda: js("return !document.querySelector('dialog') && !document.querySelector('textarea').readOnly"), 'resolution finished')

        def conflict(name, local, disk=b'# External winner\n'):
            open_note(name)
            edit(local)
            external(name, disk)
            wait(lambda: state() == 'conflict', 'conflict paused')
            assert source() == local and (library/name).read_bytes() == disk

        with (sandbox/'driver.log').open('w+') as log:
            process = subprocess.Popen(['xvfb-run','-a',DRIVER,'--port',str(dport),'--native-port',str(nport)], env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
            try:
                wait(lambda: driver.request('GET','/status'), 'driver ready')
                driver.start()
                wait(lambda: js("return !!document.querySelector('[data-testid=root-path]')"), 'onboarding')
                driver.fill('[data-testid=root-path]', str(library))
                click('select-library')
                open_note('clean.md')
                external('clean.md', b'# Clean external refresh\n')
                wait(lambda: source() == '# Clean external refresh\n', 'clean external reload')
                assert state() == 'clean'
                scenarios.append('clean external update reloads selected path')

                conflict('conflict.md', '# Local retained\n')
                choice('reload')
                finish(cancel=True)
                assert source() == '# Local retained\n' and state() == 'conflict'
                choice('reload')
                external('conflict.md', b'# Changed during reload\n')
                finish()
                assert source() == '# Local retained\n' and state() == 'conflict'
                assert js('return document.body.innerText.includes("changed again")')
                choice('reload')
                finish()
                assert source() == '# Changed during reload\n' and state() == 'clean'
                scenarios.append('explicit discard cancellation, repeated-change guard and reload')

                conflict('copy.md', '# Local copy\nOriginal\nEnd\n')
                choice('copy')
                external('copy.md', b'# Changed during copy choice\n')
                finish('unused.md')
                assert not (library/'unused.md').exists() and state() == 'conflict'
                choice('copy')
                finish('occupied.md')
                assert (library/'occupied.md').read_bytes() == b'# Occupied\n'
                assert source() == '# Local copy\nOriginal\nEnd\n' and state() == 'conflict'
                choice('copy')
                finish('recovered/local.md')
                assert (library/'copy.md').read_bytes() == b'# Changed during copy choice\n'
                assert (library/'recovered/local.md').read_bytes() == prefix + b'# Local copy\r\nOriginal\nEnd\r\n'
                assert state() == 'clean'
                scenarios.append('copy repeated-change guard, collision, exact original YAML/BOM/mixed-newline preservation')

                open_note('move.md')
                edit('# Local before move\n')
                moved_bytes = (library/'move.md').read_bytes()
                external('move.md', destination='moved.md')
                wait(lambda: state() == 'missing_on_disk', 'external move leaves missing path')
                assert source() == '# Local before move\n' and not (library/'move.md').exists()
                assert js('return document.querySelector(".metadata").textContent.includes("move.md")')
                choice('copy')
                finish('move.md')
                assert state() == 'missing_on_disk' and not (library/'move.md').exists()
                choice('copy')
                finish('recovered/moved-local.md')
                assert not (library/'move.md').exists() and (library/'moved.md').read_bytes() == moved_bytes
                assert (library/'recovered/moved-local.md').read_bytes() == prefix + b'# Local before move\r\n'
                open_note('moved.md')
                assert 'Original' in source()
                scenarios.append('external rename never retargets; missing source cannot be recreated even by copy')

                open_note('delete.md')
                edit('# Local before delete\n')
                external('delete.md')
                wait(lambda: state() == 'missing_on_disk', 'deleted note paused')
                choice('reload')
                external('delete.md', b'# Reappeared\n')
                finish()
                assert source() == '# Local before delete\n' and state() == 'conflict'
                external('delete.md')
                choice('reload')
                finish()
                assert not (library/'delete.md').exists()
                assert js('return document.querySelector("[data-testid=save-status]").hidden')
                scenarios.append('delete/reappearance guard and explicit missing-buffer discard')

                open_note('saving.md')
                js("""const original=window.fetch;
                    window.fetch=function(url,options){
                      const result=original.call(this,url,options);
                      if(String(url)!==window.__TAURI_INTERNALS__.convertFileSrc('save_note','ipc')) return result;
                      return result.then(response=>new Promise(resolve=>{window.releaseSave=()=>resolve(response);}));
                    }; window.restoreInvoke=()=>{window.fetch=original;};""")
                edit('# Committed first edit\n')
                wait(lambda: js('return typeof window.releaseSave === "function"'), 'real save committed, acknowledgement held')
                assert state() == 'saving' and b'Committed first edit' in (library/'saving.md').read_bytes()
                edit('# Newer local edit\n')
                external('saving.md', b'# External after commit\n')
                # Watcher can observe while the real acknowledgement is withheld.
                wait(lambda: 'saving.md' in js('return document.querySelector(".metadata").textContent'), 'same selected saving path')
                js('window.restoreInvoke();window.releaseSave()')
                wait(lambda: state() == 'conflict', 'in-flight newer edit paused')
                assert source() == '# Newer local edit\n' and (library/'saving.md').read_bytes() == b'# External after commit\n'
                choice('copy')
                finish('recovered/saving-local.md')
                assert (library/'saving.md').read_bytes() == b'# External after commit\n'
                assert b'Newer local edit' in (library/'recovered/saving-local.md').read_bytes()
                scenarios.append('real save acknowledgement barrier plus external writer preserves newer local and disk versions')

                open_note('saving-close.md')
                js("""const original=window.fetch;
                    window.fetch=function(url,options){
                      const result=original.call(this,url,options);
                      if(String(url)!==window.__TAURI_INTERNALS__.convertFileSrc('save_note','ipc')) return result;
                      return result.then(response=>new Promise(resolve=>{window.releaseSave=()=>resolve(response);}));
                    }; window.releaseSave=null;window.restoreInvoke=()=>{window.fetch=original;};""")
                edit('# Only retained local version\n')
                wait(lambda: js('return typeof window.releaseSave === "function"'), 'close save acknowledgement held')
                external('saving-close.md', b'# External before close acknowledgement\n')
                pid = app_pid(env['HOME'])
                native_close(pid)
                wait(lambda: js('return document.querySelector("textarea").readOnly'), 'native close awaits save')
                js('window.restoreInvoke();window.releaseSave()')
                wait(lambda: js("return !!document.querySelector('dialog[open]')"), 'native close blocked by verification conflict')
                assert source() == '# Only retained local version\n' and state() == 'conflict'
                assert (library/'saving-close.md').read_bytes() == b'# External before close acknowledgement\n'
                finish(cancel=True)
                choice('copy')
                finish('recovered/close-local.md')
                assert (library/'recovered/close-local.md').read_bytes() == prefix + b'# Only retained local version\r\n'
                scenarios.append('in-flight external replacement without newer typing retains local source and blocks native close')

                open_note('sync.md')
                sync_bytes = (library/'sync.md').read_bytes()
                external('sync (conflicted copy).md', sync_bytes)
                wait(lambda: js('return !!document.querySelector(\'[data-note-path="sync (conflicted copy).md"]\')'), 'identical sync conflict copy discovered')
                assert js('return document.querySelector(".metadata").textContent.includes("sync.md")')
                assert (library/'sync.md').read_bytes() == (library/'sync (conflicted copy).md').read_bytes()
                open_note('sync (conflicted copy).md')
                edit('# Independent sync copy\n')
                wait(lambda: state() == 'clean', 'independent conflict copy saved')
                assert (library/'sync.md').read_bytes() == sync_bytes
                assert b'Independent sync copy' in (library/'sync (conflicted copy).md').read_bytes()
                scenarios.append('identical externally synchronized copies remain distinct selectable editable paths (simulation)')

                edit('# Copy edited before close\n')
                pid = app_pid(env['HOME'])
                native_close(pid)
                wait(lambda: not Path(f'/proc/{pid}').exists(), 'resolved note flushes on native close')
                assert b'Copy edited before close' in (library/'sync (conflicted copy).md').read_bytes()
                driver.session = None
                scenarios.append('resolved editor resumes guarded autosave and native close flush')
                for scenario in scenarios:
                    print('PASS: '+scenario)
                print(f'PASS: {len(scenarios)} Phase 6 native scenarios')
            except Exception:
                log.flush()
                log.seek(0)
                print(log.read())
                if driver.session:
                    print(js('return document.body.innerText'))
                raise
            finally:
                os.killpg(process.pid, signal.SIGTERM)
                try:
                    process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    os.killpg(process.pid, signal.SIGKILL)
                    process.wait()


if __name__ == '__main__':
    main()
