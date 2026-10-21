#!/usr/bin/env python3
"""Build only search metadata; never read entry bodies or reference Simai."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import tempfile


def bounded(value, maximum, label):
    if not isinstance(value, str) or len(value.encode("utf-8")) > maximum:
        raise ValueError(f"{label}: expected string <= {maximum} UTF-8 bytes")
    return value


def identifier(value):
    bounded(value, 96, "id")
    if not re.fullmatch(r"[a-z0-9-]+", value):
        raise ValueError(f"Invalid id: {value!r}")
    return value


def file_ref(root, relative):
    path = Path(bounded(relative, 1024, "path"))
    if path.is_absolute() or not path.parts or ".." in path.parts:
        raise ValueError("Expected relative path without traversal")
    resolved = (root / path).resolve(strict=True)
    if not resolved.is_relative_to(root) or not resolved.is_file():
        raise ValueError(f"File escapes library or is not regular: {relative}")
    return {"path": relative, "bytes": resolved.stat().st_size}


def build(root):
    root = Path(root).resolve(strict=True)
    entries = []
    ids = set()
    for path in sorted((root / "entries").glob("*.json")):
        if path.stat().st_size > 64 * 1024:
            raise ValueError(f"Oversized metadata: {path}")
        item = json.loads(path.read_text(encoding="utf-8"))
        expected = {"id", "name", "aliases", "tags", "summary", "completeness", "source", "body", "references"}
        if set(item) != expected:
            raise ValueError(f"{path}: expected keys {sorted(expected)}")
        key = identifier(item["id"])
        if key in ids:
            raise ValueError(f"Duplicate id: {key}")
        ids.add(key)
        for field, size in [("name", 128), ("summary", 512), ("source", 256)]:
            bounded(item[field], size, field)
        if item["completeness"] not in {"complete", "partial", "name_only"}:
            raise ValueError(f"{key}: invalid completeness")
        for field in ("aliases", "tags"):
            if not isinstance(item[field], list) or len(item[field]) > 16:
                raise ValueError(f"{key}: {field} must be list <=16")
            for value in item[field]:
                bounded(value, 128, field)
        item["body"] = file_ref(root, item["body"])
        if not isinstance(item["references"], list) or len(item["references"]) > 32:
            raise ValueError(f"{key}: references must be list <=32")
        seen = {"entry"}
        for reference in item["references"]:
            if set(reference) != {"id", "label", "kind", "path"}:
                raise ValueError(f"{key}: invalid reference fields")
            ref_id = identifier(reference["id"])
            if ref_id in seen:
                raise ValueError(f"{key}: duplicate or reserved reference id")
            seen.add(ref_id)
            bounded(reference["label"], 128, "label")
            bounded(reference["kind"], 32, "kind")
            reference["file"] = file_ref(root, reference.pop("path"))
        entries.append(item)
    if len(entries) > 50_000:
        raise ValueError("Library exceeds 50,000 entries")
    canonical = json.dumps(entries, ensure_ascii=False, sort_keys=True, separators=(",", ":")).encode()
    index = {"schema_version": 1, "revision": hashlib.sha256(canonical).hexdigest(), "entries": entries}
    result = (json.dumps(index, ensure_ascii=False, indent=2) + "\n").encode()
    if len(result) > 32 * 1024 * 1024:
        raise ValueError("Index exceeds 32 MiB")
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    result = build(args.root)
    output = args.root / "catalog.json"
    previous = output.read_bytes() if output.exists() else None
    if args.check:
        if previous != result:
            parser.exit(1, "catalog.json is stale; run build_index.py\n")
    elif previous != result:
        with tempfile.NamedTemporaryFile(dir=args.root, prefix=".catalog-", delete=False) as stream:
            temp = Path(stream.name)
            stream.write(result)
        try:
            temp.replace(output)
        finally:
            temp.unlink(missing_ok=True)
    count = len(json.loads(result)["entries"])
    print(f"Library index: {count} entries, {len(result)} bytes" + (" (verified)" if args.check else ""))


if __name__ == "__main__":
    main()
