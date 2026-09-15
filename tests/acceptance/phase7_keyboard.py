#!/usr/bin/env python3
"""Actual Linux WebKit keyboard-only create/find/edit/move acceptance.

Build the integrated desktop first. FOGLIO_DESKTOP_BINARY selects an installed
executable too (inherited from phase4.Driver). No JS clicks/focus/edits or mocked
IPC: execute/sync is read-only observation; all interaction uses W3C key actions.
"""
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import tempfile

from phase4 import BINARY, DRIVER, Driver, port, wait

TAB, ENTER, ESCAPE, DOWN, CTRL = '\ue004', '\ue007', '\ue00c', '\ue015', '\ue009'


def main():
    assert BINARY.is_file(), f'Build the integrated desktop first: {BINARY}'
    assert all(shutil.which(tool) for tool in (DRIVER, 'xvfb-run', 'WebKitWebDriver'))
    with tempfile.TemporaryDirectory(prefix='foglio-phase7-keyboard-') as directory:
        sandbox = Path(directory)
        library = sandbox / 'notes'
        library.mkdir()
        env = {key: os.environ[key] for key in ('PATH', 'LANG', 'LD_LIBRARY_PATH') if key in os.environ}
        for key, folder in [('HOME', 'home'), ('XDG_CONFIG_HOME', 'config'), ('XDG_CACHE_HOME', 'cache'), ('XDG_DATA_HOME', 'data'), ('XDG_RUNTIME_DIR', 'runtime')]:
            target = sandbox / folder
            target.mkdir(mode=0o700)
            env[key] = str(target)
        env['WEBKIT_DISABLE_DMABUF_RENDERER'] = '1'
        dport, nport = port(), port()
        driver = Driver(f'http://127.0.0.1:{dport}')
        js = driver.js

        def keys(text, control=False, shift=False):
            actions = [{'type': 'keyDown', 'value': CTRL}] if control else []
            if shift:
                actions.append({'type': 'keyDown', 'value': '\ue008'})
            for char in text:
                actions.extend([{'type': 'keyDown', 'value': char}, {'type': 'keyUp', 'value': char}])
            if shift:
                actions.append({'type': 'keyUp', 'value': '\ue008'})
            if control:
                actions.append({'type': 'keyUp', 'value': CTRL})
            driver.request('POST', f'/session/{driver.session}/actions', {
                'actions': [{'type': 'key', 'id': 'keyboard', 'actions': actions}]})

        def focused(selector):
            return js('return document.activeElement.matches(arguments[0])', selector)

        def tab_to(selector):
            for _ in range(80):
                if focused(selector):
                    return
                keys(TAB)
            raise AssertionError(f'Not keyboard-reachable: {selector}; active=' + str(js('return document.activeElement.outerHTML')))

        def focus_result():
            for _ in range(3):
                keys(DOWN)
                if focused('.note-card'):
                    return
            raise AssertionError('Could not focus the first search result; active=' + str(js('return document.activeElement.outerHTML')))

        def saved():
            return js("return document.querySelector('[data-testid=save-status]').dataset.state==='clean'")

        def await_save():
            # Busy is an explicit retained-buffer outcome, not an automatic IPC retry.
            wait(lambda: saved() or js("return document.querySelector('[data-testid=save-status]').dataset.state==='save_error'"), 'save outcome')
            if not saved():
                assert js("return document.querySelector('[data-testid=save-error]').textContent.startsWith('busy:')"), js("return document.querySelector('[data-testid=save-error]').textContent")
                tab_to('.reader > button')  # The visible, explicitly labelled Retry save control.
                assert js("return document.activeElement.textContent==='Retry save'")
                keys(ENTER)
                wait(saved, 'explicit Retry save')

        with (sandbox / 'driver.log').open('w+') as log:
            process = subprocess.Popen(['xvfb-run', '-a', DRIVER, '--port', str(dport), '--native-port', str(nport)], env=env, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
            try:
                wait(lambda: driver.request('GET', '/status'), 'driver ready')
                driver.start()
                wait(lambda: js("return !!document.querySelector('[data-testid=root-path]')"), 'onboarding')
                tab_to('[data-testid=root-path]')
                keys(str(library))
                keys(ENTER)
                wait(lambda: js("return document.querySelector('.status').textContent==='Monitoring external changes' && !!document.querySelector('nav button')"), 'library loaded')

                keys('n', control=True)
                wait(lambda: focused('[data-testid=operation-value]'), 'title autofocus')
                assert js("const d=document.querySelector('dialog');return !!document.getElementById(d.getAttribute('aria-labelledby')) && !!document.getElementById(d.getAttribute('aria-describedby'))")
                keys('Keyboard fixture')
                keys(ENTER)
                created = library / 'Keyboard-fixture.md'
                wait(lambda: created.exists() and focused('textarea') and js("return !document.querySelector('textarea').readOnly"), 'created and editor focused')
                assert created.read_text() == '# Keyboard fixture\n'
                assert js("""const button = action => document.querySelector(`[data-testid=format-${action}]`);
                    return button('bold').title === 'Bold (Ctrl+B)'
                      && button('italic').title === 'Italic (Ctrl+I)'
                      && button('link').title === 'Link (Ctrl+K)'
                      && button('strike').title === 'Strikethrough';"""), 'formatting toolbar tooltips must include available shortcuts'
                keys('a', control=True)
                keys('keyboardneedle body')
                keys('s', control=True)
                await_save()
                assert created.read_text() == 'keyboardneedle body'

                keys('z', control=True)
                wait(lambda: js("return document.querySelector('textarea').value!=='keyboardneedle body'"), 'native undo after save')
                undone = js("return document.querySelector('textarea').value")
                await_save()
                assert created.read_text() == undone
                keys('z', control=True, shift=True)
                wait(lambda: js("return document.querySelector('textarea').value==='keyboardneedle body'"), 'native redo after save')
                await_save()
                assert created.read_text() == 'keyboardneedle body'
                keys('z', control=True)
                wait(lambda: js("return document.querySelector('textarea').value===arguments[0]", undone), 'second undo')
                keys('e', control=True)
                keys('e', control=True)
                keys('y', control=True)
                wait(lambda: js("return document.querySelector('textarea').value==='keyboardneedle body'"), 'Ctrl+Y redo across mode switch')
                await_save()
                assert created.read_text() == 'keyboardneedle body'

                keys('f', control=True)
                assert focused('[aria-label="Search notes"]')
                keys('keyboardneedle')
                wait(lambda: js("return document.querySelectorAll('.note-card').length===1 && document.querySelector('.note-card').dataset.notePath==='Keyboard-fixture.md'"), 'literal search finds saved body')
                focus_result()
                keys(ENTER)
                wait(lambda: js("return document.querySelector('.metadata').textContent.includes('Keyboard-fixture.md') && !document.querySelector('textarea').readOnly"), 'result opened')
                keys('e', control=True)
                assert focused('[aria-label="Note preview"]')
                keys('e', control=True)
                assert focused('[aria-label="Markdown source (body only)"]')
                keys('z', control=True)
                assert js("return document.querySelector('textarea').value==='keyboardneedle body'"), {'check': 'reopening resets history', 'source': js("return document.querySelector('textarea').value"), 'focus': js('return document.activeElement.outerHTML')}
                keys('a', control=True)
                keys('keyboardneedle edited')
                keys('s', control=True)
                await_save()
                assert created.read_text() == 'keyboardneedle edited', {'disk': created.read_text(), 'source': js("return document.querySelector('textarea').value"), 'focus': js('return document.activeElement.outerHTML')}

                keys('f', control=True)
                focus_result()
                wait(lambda: focused('.note-card'), 'search result focused before header actions')
                tab_to('[data-testid=note-actions-trigger]')
                keys(ENTER)
                wait(lambda: focused('[data-testid=move-note]'), 'actions menu opens on Move / rename')
                keys(ENTER)
                wait(lambda: focused('[data-testid=operation-value]'), 'move path autofocus')
                keys(ESCAPE)
                wait(lambda: focused('[data-testid=note-actions-trigger]'), 'cancel restores actions-menu focus')
                keys(ENTER)
                wait(lambda: focused('[data-testid=move-note]'), 'actions menu reopened')
                keys(ENTER)
                wait(lambda: focused('[data-testid=operation-value]'), 'move dialog reopened')
                keys('a', control=True)
                keys('archive/renamed.md')
                keys(ENTER)
                moved = library / 'archive/renamed.md'
                wait(lambda: moved.exists() and not created.exists(), 'move committed')
                wait(lambda: js("return !document.querySelector('textarea').readOnly && document.querySelector('.metadata').textContent.includes('archive/renamed.md')"), 'moved note loaded')
                assert moved.read_bytes() == b'keyboardneedle edited'

                tab_to('[data-testid=keyboard-help]')
                keys(ENTER)
                wait(lambda: js("return !!document.querySelector('dialog[open]')"), 'help opens')
                before = js("return document.querySelector('textarea').hidden")
                keys('e', control=True)
                keys('n', control=True)
                assert js("return document.querySelectorAll('dialog').length===1")
                assert before == js("return document.querySelector('textarea').hidden")
                keys(ESCAPE)
                assert focused('[data-testid=keyboard-help]')
                assert moved.read_bytes() == b'keyboardneedle edited'
                print(json.dumps({'binary': str(BINARY.resolve()), 'passed': ['keyboard-only onboarding/create/find/edit/move', 'native dialog Escape and focus restoration', 'help discoverability and modal shortcut isolation', 'native keyboard undo/redo, autosave, mode-switch history and reopen isolation', 'accessible labels and exact saved/moved bytes']}))
            except Exception:
                log.flush()
                log.seek(0)
                print(log.read())
                raise
            finally:
                try:
                    driver.close()
                finally:
                    os.killpg(process.pid, signal.SIGTERM)
                    process.wait(timeout=10)


if __name__ == '__main__':
    main()
