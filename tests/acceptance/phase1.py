#!/usr/bin/env python3
"""Independent Phase 1 acceptance against the real compiled notes executable.

Run: cargo build --locked --workspace && python3 tests/acceptance/phase1.py
All note and configuration state lives in disposable temporary directories.
"""

import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


BINARY = Path(__file__).resolve().parents[2] / "target/debug/notes"


class Phase1(unittest.TestCase):
    def setUp(self):
        self.sandbox = tempfile.TemporaryDirectory(prefix="foglio-acceptance-")
        self.addCleanup(self.sandbox.cleanup)
        self.base = Path(self.sandbox.name)
        self.library = self.base / "library"
        self.library.mkdir()
        self.env = {"NO_COLOR": "1", "PATH": os.defpath}
        for variable, directory in (
            ("HOME", "home"),
            ("XDG_CONFIG_HOME", "config"),
            ("XDG_CACHE_HOME", "cache"),
            ("XDG_DATA_HOME", "data"),
        ):
            path = self.base / directory
            path.mkdir()
            self.env[variable] = str(path)

    def run_notes(self, *args, success=True):
        result = subprocess.run(
            [str(BINARY), *map(str, args)],
            cwd=self.library,
            env=self.env,
            input=b"",
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=15,
            check=False,
        )
        if success:
            self.assertEqual(result.returncode, 0, (args, result.stdout, result.stderr))
        else:
            self.assertNotEqual(result.returncode, 0, (args, result.stdout, result.stderr))
        return result

    def manifest(self):
        return {
            str(path.relative_to(self.library)): path.read_bytes()
            for path in self.library.rglob("*")
            if path.is_file()
        }

    def test_lifecycle_and_state_deletion(self):
        imported = self.library / "import.md"
        original = b"# Imported\n\nArbitrary **Markdown**, [[wiki]], and ![image](x).\n"
        imported.write_bytes(original)
        self.run_notes("init", self.library)
        adopted = imported.read_bytes()
        self.assertEqual(adopted, original)
        self.run_notes("init", self.library)
        self.assertEqual(imported.read_bytes(), adopted)

        self.run_notes("new", "Acceptance note", "--path", "fresh.md")
        fresh = self.library / "fresh.md"
        fresh_path = "fresh.md"
        self.assertEqual(b"# Acceptance note\n", fresh.read_bytes())
        self.assertEqual(self.run_notes("show", fresh_path).stdout, fresh.read_bytes())
        listing = self.run_notes("--json", "list")
        self.assertIsInstance(json.loads(listing.stdout), dict)
        self.run_notes("tag", "add", fresh_path, "CaseSensitive")
        once = fresh.read_bytes()
        self.run_notes("tag", "add", fresh_path, "CaseSensitive")
        self.assertEqual(fresh.read_bytes(), once)
        self.assertIn(b"CaseSensitive", self.run_notes("tags").stdout)
        self.run_notes("tag", "remove", fresh_path, "missing")
        self.assertEqual(fresh.read_bytes(), once)
        self.run_notes("tag", "remove", fresh_path, "CaseSensitive")

        before_move = fresh.read_bytes()
        self.run_notes("move", fresh_path, "nested/moved.md")
        moved = self.library / "nested/moved.md"
        self.assertFalse(fresh.exists())
        self.assertEqual(moved.read_bytes(), before_move)
        fresh_path = "nested/moved.md"
        self.assertEqual(self.run_notes("show", "nested/moved.md").stdout, before_move)

        before_reset = self.manifest()
        for name in ("config", "cache", "data"):
            shutil.rmtree(self.base / name)
            (self.base / name).mkdir()
        self.run_notes("init", self.library)
        self.assertEqual(self.manifest(), before_reset)
        self.run_notes("delete", fresh_path, success=False)
        self.assertEqual(moved.read_bytes(), before_move)
        self.run_notes("delete", fresh_path, "--yes")
        self.assertFalse(moved.exists())
        self.assertEqual(imported.read_bytes(), adopted)


    def test_preservation_of_nested_yaml_bom_crlf_and_tags(self):
        note = self.library / "preserved.md"
        prefix = (
            b"\xef\xbb\xbf---\r\n"
            b"# User-owned comment\r\n"
            b"extra:\r\n  id: untouched\r\n  nested: [one, two]\r\n"
            b"description: |\r\n  tags: [not-app-tags]\r\n"
            b"tags: [\"a:b\", \"#literal\"]\r\n"
        )
        body = b"\r\n```md\r\n# Not the title\r\n---\r\n```\r\n# Actual title\r\nNo final newline"
        note.write_bytes(prefix + b"---\r\n" + body)
        self.run_notes("init", self.library)
        adopted = note.read_bytes()
        self.assertTrue(adopted.startswith(b"\xef\xbb\xbf---\r\n"))
        self.assertTrue(adopted.endswith(body))
        for retained in (
            b"# User-owned comment\r\n",
            b"extra:\r\n  id: untouched\r\n  nested: [one, two]\r\n",
            b"description: |\r\n  tags: [not-app-tags]\r\n",
        ):
            self.assertIn(retained, adopted)
        self.assertEqual(adopted, prefix + b"---\r\n" + body)
        note_path = "preserved.md"
        self.run_notes("tag", "add", note_path, "new-tag")
        edited = note.read_bytes()
        self.assertTrue(edited.endswith(body))
        self.assertIn(b"description: |\r\n  tags: [not-app-tags]\r\n", edited)
        self.assertIn(b"extra:\r\n  id: untouched\r\n  nested: [one, two]\r\n", edited)
        self.assertIn(b"Actual title", self.run_notes("list").stdout)

    def test_invalid_metadata_is_never_modified(self):
        fixtures = {
            "duplicate.md": b"---\nid: null\nid: null\n---\nbody\n",
            "nested-duplicate.md": b"---\ncustom:\n  key: a\n  'key': b\n---\nbody\n",
            "wrong-tags.md": b"---\ntags: scalar\n---\nbody\n",
            "bad-id.md": b"---\nid: 80000000000000000000000000\n---\nbody\n",
            "null-id.md": b"---\nid: null\n---\nbody\n",
            "unterminated.md": b"---\ntags: []\nbody\n",
            "invalid-utf8.md": b"# Body\n\xff\n",
        }
        for name, content in fixtures.items():
            (self.library / name).write_bytes(content)
        before = self.manifest()
        result = self.run_notes("init", self.library, success=False)
        self.assertIn(b"invalid-utf8.md", result.stderr + result.stdout)
        self.assertIn(b"nested-duplicate.md", result.stderr + result.stdout)
        self.assertEqual(self.manifest(), before)

    def test_hidden_discovery_and_existing_read_only_notes(self):
        hidden = self.library / ".hidden"
        hidden.mkdir()
        note = hidden / "note.md"
        note.write_bytes(b"# Hidden note\n")
        non_note = self.library / "ordinary.txt"
        non_note.write_bytes(b"Not a Markdown note\n")
        readonly = self.library / "readonly.md"
        original = b"# Read-only import must not be adopted\n"
        readonly.write_bytes(original)
        readonly.chmod(0o444)
        self.run_notes("init", self.library, success=False)
        self.assertEqual(note.read_bytes(), b"# Hidden note\n")
        self.assertEqual(readonly.read_bytes(), original)
        self.assertEqual(non_note.read_bytes(), b"Not a Markdown note\n")
        non_note.unlink()
        readonly.unlink()
        self.run_notes("init", self.library)
        adopted = note.read_bytes()
        note.chmod(0o444)
        self.run_notes("init", self.library)
        self.assertEqual(note.read_bytes(), adopted)
        self.assertEqual(self.run_notes("show", ".hidden/note.md").stdout, adopted)

    def test_unsafe_alias_mutations_leave_all_bytes_unchanged(self):
        note = self.library / "alias.md"
        content = (
            b"---\nid: 01ARZ3NDEKTSV4RRFFQ69G5FAV\n"
            b"tags: &shared [one]\ncustom: *shared\n---\n# Alias\n"
        )
        note.write_bytes(content)
        self.run_notes("--library", self.library, "tag", "add", "alias.md", "two", success=False)
        self.assertEqual(note.read_bytes(), content)

    def test_literal_tag_punctuation_and_comment_preservation(self):
        self.run_notes("init", self.library)
        self.run_notes("new", "Tags", "--path", "tags.md")
        note = self.library / "tags.md"
        for tag in ("a*b", "a&b", "important!", "colon: value", "#literal", "雪"):
            self.run_notes("tag", "add", "tags.md", tag)
            self.assertIn(tag.encode(), self.run_notes("tags").stdout)
        lines = note.read_bytes().splitlines(keepends=True)
        for index, line in enumerate(lines):
            if line.startswith(b"tags: "):
                lines[index] = line.rstrip(b"\n") + b" # preserve comment\n"
                break
        annotated = b"".join(lines)
        note.write_bytes(annotated)
        result = subprocess.run(
            [str(BINARY), "tag", "add", "tags.md", "extra"],
            cwd=self.library, env=self.env, input=b"", capture_output=True,
            timeout=15, check=False,
        )
        if result.returncode == 0:
            self.assertIn(b"# preserve comment", note.read_bytes())
        else:
            self.assertEqual(note.read_bytes(), annotated)

    def test_duplicate_metadata_and_identical_content_are_independent_paths(self):
        first = b"---\nid: arbitrary # user-owned\n---\n# First\n"
        for name in ("a.md", "b.md"):
            (self.library / name).write_bytes(first)
        before = self.manifest()
        self.run_notes("init", self.library)
        self.assertEqual(self.manifest(), before)
        listing = json.loads(self.run_notes("--json", "list").stdout)["result"]
        self.assertEqual([entry["path"] for entry in listing], ["a.md", "b.md"])
        self.assertTrue(all("id" not in entry for entry in listing))
        self.assertEqual(self.run_notes("show", "a.md").stdout, first)
        self.run_notes("show", "id:arbitrary", success=False)
        self.run_notes("tag", "add", "a.md", "work")
        self.assertIn(b"id: arbitrary # user-owned", (self.library / "a.md").read_bytes())
        self.assertEqual((self.library / "b.md").read_bytes(), first)
        self.run_notes("delete", "b.md", "--yes")
        self.assertTrue((self.library / "a.md").exists())

    def test_destinations_never_clobber_or_escape(self):
        self.run_notes("init", self.library)
        self.run_notes("new", "Original", "--path", "a.md")
        self.run_notes("new", "Occupied", "--path", "b.md")
        before = self.manifest()
        self.run_notes("new", "Overwrite", "--path", "a.md", success=False)
        self.run_notes("move", "a.md", "b.md", success=False)
        for target in ("../escape.md", str(self.base / "absolute.md"), "nested/../escape.md"):
            self.run_notes("move", "a.md", target, success=False)
            self.run_notes("new", "Escape", "--path", target, success=False)
        self.assertEqual(self.manifest(), before)
        self.assertFalse((self.base / "escape.md").exists())
        self.assertFalse((self.base / "absolute.md").exists())

    def test_acl_and_different_group_never_broaden_access(self):
        import struct
        self.run_notes("init", self.library)
        self.run_notes("new", "Private", "--path", "private.md")
        note = self.library / "private.md"
        before = note.read_bytes()
        # Linux POSIX ACL xattr, synthetic test data: a named user entry.
        acl = struct.pack("<I", 2) + b"".join(
            struct.pack("<HHI", tag, permissions, identity)
            for tag, permissions, identity in (
                (1, 6, 0xFFFFFFFF), (2, 4, 1), (4, 0, 0xFFFFFFFF),
                (16, 4, 0xFFFFFFFF), (32, 0, 0xFFFFFFFF),
            )
        )
        os.setxattr(note, "system.posix_acl_access", acl)
        result = self.run_notes("tag", "add", "private.md", "no", success=False)
        self.assertIn(b"unsupported", result.stderr)
        self.assertEqual(note.read_bytes(), before)
        self.assertEqual(os.getxattr(note, "system.posix_acl_access"), acl)
        os.removexattr(note, "system.posix_acl_access")
        groups = [group for group in os.getgroups() if group != os.getgid()]
        if groups:
            os.chown(note, -1, groups[0])
            note.chmod(0o640)
            self.run_notes("tag", "add", "private.md", "no", success=False)
            self.assertEqual(note.stat().st_gid, groups[0])
            self.assertEqual(note.read_bytes(), before)

    def test_interactive_delete_cancellation_and_stale_confirmation(self):
        import pty
        import select
        import time
        self.run_notes("init", self.library)
        self.run_notes("new", "Prompt", "--path", "prompt.md")
        note = self.library / "prompt.md"
        original = note.read_bytes()

        def confirm(answer, external=None):
            master, slave = pty.openpty()
            child = subprocess.Popen(
                [str(BINARY), "delete", "prompt.md"], cwd=self.library,
                env=self.env, stdin=slave, stderr=slave, stdout=subprocess.PIPE,
            )
            os.close(slave)
            try:
                prompt = b""
                deadline = time.monotonic() + 10
                while b"Type yes:" not in prompt and time.monotonic() < deadline:
                    if select.select([master], [], [], 0.1)[0]:
                        prompt += os.read(master, 4096)
                self.assertIn(b"Type yes:", prompt)
                if external is not None:
                    note.write_bytes(external)
                os.write(master, answer + b"\n")
                child.communicate(timeout=10)
                return child.returncode
            finally:
                if child.poll() is None:
                    child.kill()
                    child.wait()
                os.close(master)

        self.assertEqual(confirm(b"no"), 1)
        self.assertEqual(note.read_bytes(), original)
        external = original + b"External edit while confirmation was open\n"
        self.assertEqual(confirm(b"yes", external), 4)
        self.assertEqual(note.read_bytes(), external)
        self.assertEqual(confirm(b"yes"), 0)
        self.assertFalse(note.exists())

    def test_json_errors_and_concurrent_no_clobber(self):
        self.run_notes("init", self.library)
        for args, code in ((["--json", "unknown"], 2),
                           (["--json", "show", "missing.md"], 3),
                           (["--json", "new", "bad/title"], 2)):
            result = self.run_notes(*args, success=False)
            self.assertEqual(result.returncode, code)
            self.assertEqual(result.stderr, b"")
            envelope = json.loads(result.stdout)
            self.assertEqual(set(envelope), {"result", "diagnostics", "incomplete", "error"})
            self.assertTrue(envelope["incomplete"])
            self.assertIsNotNone(envelope["error"])
        children = [subprocess.Popen(
            [str(BINARY), "new", title, "--path", "race.md"],
            cwd=self.library, env=self.env, stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        ) for title in ("First", "Second")]
        for child in children:
            child.communicate(timeout=15)
        self.assertEqual(sorted(child.returncode for child in children), [0, 4])
        self.assertNotIn(b"id:", (self.library / "race.md").read_bytes())
        body = (self.library / "race.md").read_bytes()
        self.assertTrue(body.endswith(b"# First\n") or body.endswith(b"# Second\n"))

    @unittest.skipUnless(os.name == "posix", "Linux filesystem policy")
    def test_symlinks_are_never_followed_and_hardlinks_not_written(self):
        outside = self.base / "outside"
        outside.mkdir()
        target = outside / "secret.md"
        sentinel = b"# Outside must remain untouched\n"
        target.write_bytes(sentinel)
        (self.library / "link.md").symlink_to(target)
        (self.library / "linked-directory").symlink_to(outside, target_is_directory=True)
        self.run_notes("init", self.library, success=False)
        for selector in ("link.md", "linked-directory/secret.md", "../outside/secret.md"):
            output = self.run_notes("--library", self.library, "show", selector, success=False)
            self.assertNotIn(sentinel, output.stdout)
        self.run_notes("--library", self.library, "new", "Escape", "--path", "linked-directory/new.md", success=False)
        self.assertFalse((outside / "new.md").exists())
        self.assertEqual(target.read_bytes(), sentinel)
        os.link(target, self.library / "hard.md")
        self.run_notes("init", self.library, success=False)
        self.assertEqual(target.read_bytes(), sentinel)
        self.assertEqual((self.library / "hard.md").read_bytes(), sentinel)


if __name__ == "__main__":
    if not BINARY.is_file():
        raise SystemExit("Build first: cargo build --locked --workspace")
    unittest.main(verbosity=2)
