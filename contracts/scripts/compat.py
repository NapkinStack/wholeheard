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

import os
import sys
from pathlib import Path

import jsoncompat


def schemas(folder):
    """A version's schemas by name — and the files that look like one where none is read."""
    folder = Path(folder)
    read = {path.name: path for path in sorted(folder.glob("*.schema.json")) if path.is_file()}
    stray = {*folder.rglob("*.json"), *folder.rglob("*.schema.*")} - set(read.values())
    return read, sorted(path.relative_to(folder).as_posix() for path in stray)


def judged(merged, changed):
    """Nothing when the changed schema is compatible with the merged one; why not, otherwise."""
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
        refused.append(
            "no schema in this version: nothing can be compared — give it one first, in a pull request of its own"
        )
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
