#!/usr/bin/env python3
"""Phase 2 black-box acceptance; only isolated synthetic temporary libraries."""
import concurrent.futures
from contextlib import contextmanager

@contextmanager
def database(path):
    db = sqlite3.connect(path)
    try:
        with db:
            yield db
    finally:
        db.close()

import fcntl
import hashlib
import json
import os
from pathlib import Path
import sqlite3
import subprocess
import tempfile
import unittest

BINARY = Path(__file__).resolve().parents[2] / "target/debug/notes"

class Phase2(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory(prefix="foglio-phase2-")
        self.addCleanup(tmp.cleanup)
        self.base = Path(tmp.name)
        self.root = self.base / "library"
        self.root.mkdir()
        self.env = {"PATH": os.defpath, "NO_COLOR": "1"}
        for var, name in [("HOME", "home"), ("XDG_CONFIG_HOME", "config"), ("XDG_CACHE_HOME", "cache")]:
            (self.base / name).mkdir()
            self.env[var] = str(self.base / name)
        self.run_cli("init", self.root)

    def run_cli(self, *args, code=0):
        out = subprocess.run([str(BINARY), "--json", *map(str,args)], env=self.env, cwd=self.root, capture_output=True, timeout=20)
        self.assertEqual(out.returncode, code, (args,out.stdout,out.stderr))
        self.assertFalse(out.stderr)
        return json.loads(out.stdout)

    def cache(self):
        return next((self.base / "cache").rglob("index.sqlite3"))

    def manifest(self):
        return {str(p.relative_to(self.root)):p.read_bytes() for p in self.root.rglob("*.md")}

    def integrity(self):
        with database(self.cache()) as db:
            self.assertEqual(db.execute("PRAGMA integrity_check").fetchall(), [("ok",)])
            self.assertEqual(db.execute("PRAGMA foreign_key_check").fetchall(), [])
            self.assertEqual(db.execute("SELECT count(*) FROM notes").fetchone(),db.execute("SELECT count(*) FROM notes_fts").fetchone())
            self.assertEqual(db.execute("SELECT count(*) FROM tags WHERE note_path NOT IN (SELECT path FROM notes)").fetchone(), (0,))

    def test_cli_search_recovery_and_external_lifecycle(self):
        self.run_cli("new", "Alpha", "--path", "foo/a.md", "--body", "# Alpha\nquick brown café", "--tag", "Case")
        self.run_cli("new", "Beta", "--path", "foobar/b.md", "--body", "quick red brown", "--tag", "case")
        self.assertEqual(self.run_cli("status")["result"]["parsed_notes"],0)
        self.assertFalse(self.run_cli("status")["result"]["watcher_active"])
        for args, count in [(('search','quick'),2),(('search','quick brown','--phrase'),1),(('search','qui','--prefix'),2),(('search','quick','--tag','Case'),1),(('search','quick','--folder','foo'),1)]:
            self.assertEqual(len(self.run_cli(*args)["result"]["hits"]),count)
        for args in [('search',''),('search','***'),('search','quick','--phrase','--prefix'),('search','quick','--limit','0'),('search','quick','--folder','../')]:
            self.run_cli(*args,code=2)
        before = self.manifest()
        self.cache().unlink()
        self.assertEqual(self.run_cli("reindex")["result"]["indexed_notes"],2)
        self.cache().write_bytes(b"corrupted database header")
        self.assertTrue(self.run_cli("reindex")["result"]["cache_rebuilt"])
        self.assertEqual(self.manifest(),before)
        p = self.root / 'foo/a.md'
        p.write_text(p.read_text().replace('quick brown','newtoken brown'))
        self.assertEqual(len(self.run_cli('search','newtoken')["result"]["hits"]),1)
        p.rename(self.root/'moved.md')
        self.assertEqual(self.run_cli('search','newtoken')["result"]["hits"][0]['path'],'moved.md')
        (self.root/'moved.md').unlink()
        self.assertEqual(self.run_cli('rescan')["result"]["indexed_notes"],1)
        self.integrity()

    def test_all_committed_mutations_survive_index_transaction_failure(self):
        self.run_cli('new','A','--path','a.md','--body','old')
        cache = self.cache()
        # Trigger failure after metadata insertion but before derived FTS publication.
        with database(cache) as db:
            db.execute("CREATE TRIGGER fail_index BEFORE INSERT ON notes BEGIN SELECT RAISE(ABORT,'injected index failure'); END")
        out = self.run_cli('tag','add','a.md','Saved',code=1)
        self.assertEqual(out['error']['code'],'committed')
        self.assertTrue(out['error']['commit']['file_committed'])
        self.assertTrue(out['error']['commit']['durability_confirmed'])
        self.assertIn('Saved',(self.root/'a.md').read_text())
        with database(cache) as db:
            self.assertEqual(db.execute('SELECT count(*) FROM tags').fetchone(),(0,))
            db.execute('DROP TRIGGER fail_index')
        self.run_cli('rescan')
        self.assertEqual(len(self.run_cli('search','Saved')['result']['hits']),1)
        for args in [('new','B','--path','b.md'),('move','a.md','moved.md'),('tag','remove','moved.md','Saved'),('delete','moved.md','--yes')]:
            cache.unlink(); cache.mkdir()
            out = self.run_cli(*args,code=1)
            self.assertEqual(out['error']['code'],'committed')
            self.assertTrue(out['error']['commit']['file_committed'])
            cache.rmdir()
            self.run_cli('reindex')
        self.assertTrue((self.root/'b.md').exists())
        self.assertFalse((self.root/'a.md').exists())
        self.assertFalse((self.root/'moved.md').exists())
        self.integrity()

    def test_unknown_subtree_preserved_until_confirmed_absent(self):
        self.run_cli('new','A','--path','foo/a.md')
        self.run_cli('new','B','--path','foobar/b.md')
        folder = self.root/'foo'
        folder.chmod(0)
        try:
            status = self.run_cli('rescan',code=1)['result']
            self.assertEqual(status['stale_notes'],1)
            self.assertTrue(status['incomplete'])
            with database(self.cache()) as db:
                self.assertEqual(db.execute('SELECT path,stale FROM files ORDER BY path').fetchall(),[('foo/a.md',1),('foobar/b.md',0)])
        finally:
            folder.chmod(0o700)
        self.assertEqual(self.run_cli('rescan')['result']['indexed_notes'],2)
        (folder/'a.md').unlink()
        self.assertEqual(self.run_cli('rescan')['result']['indexed_notes'],1)
        self.integrity()

    def test_concurrent_writers_rebuild_readers_and_bounded_busy(self):
        self.run_cli('new','Base')
        lock = next((self.base/'config').rglob('*.lock'))
        # Select root-keyed lock, not the independent selection lock.
        lock = self.base/'config/foglio/locks'/f'{hashlib.sha256(str(self.root).encode()).hexdigest()}.lock'
        with lock.open('rb') as handle:
            fcntl.flock(handle,fcntl.LOCK_EX)
            for args in [('new','Blocked'),('status',),('reindex',),('search','Base')]:
                self.assertEqual(self.run_cli(*args,code=4)['error']['code'],'busy')
        commands = [('new',f'Note-{i}') for i in range(12)] + [('reindex',),('rescan',),('search','Base'),('status',)]*3
        def run(args):
            out = subprocess.run([str(BINARY),'--json',*args],env=self.env,cwd=self.root,capture_output=True,timeout=20)
            return args,out.returncode,json.loads(out.stdout)
        with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:
            results = list(pool.map(run,commands))
        for args, code, out in results:
            self.assertIn(code,(0,4),(args,out))
            if code == 4:
                self.assertEqual(out['error']['code'],'busy')
            else:
                self.assertFalse(out['incomplete'])
        self.run_cli('reindex')
        paths = sorted(p.name for p in self.root.glob('*.md'))
        expected = ['Base.md'] + [f'{args[1]}.md' for args, code, _ in results if args[0] == 'new' and code == 0]
        self.assertEqual(paths, sorted(expected))
        self.assertTrue(all(not p.read_text().startswith('---') for p in self.root.glob('*.md')))
        self.integrity()

if __name__ == '__main__':
    unittest.main(verbosity=2)
