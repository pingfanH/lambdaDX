import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import build_index


class IndexTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        (self.root / "entries").mkdir()
        (self.root / "body.md").write_text("中文", encoding="utf-8")
        with (self.root / "reference.simai").open("wb") as file:
            file.truncate(1024**3)
        self.entry = {"id":"test", "name":"交互", "aliases":["别名"], "tags":["TAP"], "summary":"左右手交替", "completeness":"complete", "source":"fixture", "body":"body.md", "references":[{"id":"sample", "label":"参考", "kind":"simai", "path":"reference.simai"}]}
        self.write()

    def write(self):
        (self.root / "entries/test.json").write_text(json.dumps(self.entry), encoding="utf-8")

    def test_deterministic_metadata_only_and_portable(self):
        read_text = Path.read_text
        def metadata_only(path, *args, **kwargs):
            self.assertEqual(path.suffix, ".json", "Do not read bodies or Simai during indexing")
            return read_text(path, *args, **kwargs)
        with patch.object(Path, "read_text", metadata_only):
            first = build_index.build(self.root)
            self.assertEqual(first, build_index.build(self.root))
        index = json.loads(first)
        self.assertEqual(index["entries"][0]["references"][0]["file"]["bytes"], 1024**3)
        self.assertLess(len(first), 2048)
        # mtime changes after checkout must not change a portable catalog.
        (self.root / "body.md").touch()
        self.assertEqual(first, build_index.build(self.root))

    def test_invalid_paths_ids_and_incomplete_metadata(self):
        for field, value in [("id","../escape"), ("body","../escape"), ("summary","中"*200), ("completeness","invented")]:
            original = self.entry[field]
            self.entry[field] = value
            self.write()
            with self.assertRaises((ValueError, FileNotFoundError)):
                build_index.build(self.root)
            self.entry[field] = original
        self.entry["references"][0]["id"] = "entry"
        self.write()
        with self.assertRaises(ValueError):
            build_index.build(self.root)

    def test_duplicate_id_rejected(self):
        (self.root / "entries/duplicate.json").write_text(json.dumps(self.entry))
        with self.assertRaisesRegex(ValueError, "Duplicate id"):
            build_index.build(self.root)


if __name__ == "__main__":
    unittest.main()
