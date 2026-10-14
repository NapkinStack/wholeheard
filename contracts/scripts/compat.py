"""Whether a version of a contract, as changed, is compatible with the same version as merged.

`nstack compat` runs this as merged, in the merged tree, with the two folders in
NSTACK_BASE_PATH and NSTACK_HEAD_PATH (.nstack/handbook/03-contracts.md §3). It compares the
schemas a version holds — the `*.schema.json` files at its top — and nothing else: a
contract's pages are read by a person.

The rule is the consumers': everything a producer may write under the changed schema must
be something the merged schema already allowed. A field that becomes optional, is renamed
or changes type is refused; one that is added, where the merged schema left room for it, is
not. **What cannot be decided is refused**, with the comparator's reason. Unchanged
structure with a changed meaning is seen by no comparison: contract tests see it.
"""

import json
import os
import sys
from pathlib import Path

import jsoncompat

DIALECT = "https://json-schema.org/draft/2020-12/schema"


def schemas(folder):
    """A version's schemas by name — and the schemas that are where none is read: any file
    named as one, or any JSON document that names a schema's dialect, elsewhere than at the
    version's top as `<document>.schema.json`. Other JSON files are examples, and are left alone."""
    folder = Path(folder)
    read = {path.name: path for path in sorted(folder.glob("*.schema.json")) if path.is_file()}
    stray = set(folder.rglob("*.schema.*")) | {path for path in folder.rglob("*.json") if dialect(path)}
    return read, sorted(path.relative_to(folder).as_posix() for path in stray - set(read.values()))


def dialect(path):
    """The dialect a JSON document says it is a schema of, if it says one."""
    try:
        document = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return None
    return document.get("$schema") if isinstance(document, dict) else None


def judged(merged, changed):
    """Nothing when the changed schema is compatible with the merged one; why not, otherwise."""
    if dialect(changed) != DIALECT:
        return f"it does not say it is a schema of {DIALECT}, the one dialect the comparator reads"
    try:
        texts = merged.read_text(encoding="utf-8"), changed.read_text(encoding="utf-8")
        found = jsoncompat.analyze_compat(*texts, "serializer")
    except Exception as error:  # noqa: BLE001 — whatever the comparator cannot read is refused, and said
        return f"it cannot be compared ({' '.join(str(error).split())[:300]})"
    if found["status"] == "compatible":
        return None
    if found["status"] == "incompatible" and found.get("counterexample_json"):
        return f"the changed schema allows what the merged one refuses: {found['counterexample_json']}"
    reason = found.get("reason", found["status"])
    return f"the comparison could not be decided, and what is not proven is refused ({reason})"


def main():
    name = f"{os.environ.get('NSTACK_CONTRACT', 'contract')} {os.environ.get('NSTACK_VERSION', '')}".strip()
    if not all(os.environ.get(key) for key in ("NSTACK_BASE_PATH", "NSTACK_HEAD_PATH")):
        print(f"{name}: NSTACK_BASE_PATH and NSTACK_HEAD_PATH are not set — this is run by `nstack compat`")
        return 1
    (merged, _), (changed, stray) = schemas(os.environ["NSTACK_BASE_PATH"]), schemas(os.environ["NSTACK_HEAD_PATH"])
    refused = [
        f"{path}: a schema is a `*.schema.json` file at the version's top — nothing reads this one" for path in stray
    ]
    if not merged and not changed:
        refused.append("no schema in this version: nothing can be compared, and what is not proven is refused")
    for schema in sorted(merged.keys() | changed.keys()):
        if schema not in changed:
            refused.append(f"{schema}: removed — what it promised is promised by nothing")
            continue
        # A schema is compared with what was merged — or, the first time, with itself: one
        # the comparator cannot read would freeze its version for good.
        why = judged(merged.get(schema, changed[schema]), changed[schema])
        if why:
            refused.append(f"{schema}: {why}")
        else:
            print(
                f"{name}: {schema} {'compatible' if schema in merged else 'is new, and reads'} — pages are not compared"
            )
    for line in refused:
        print(f"{name}: {line}")
    return 1 if refused else 0


if __name__ == "__main__":
    sys.exit(main())
