"""Whether a version of a contract, as changed, is compatible with the same version as merged.

`nstack compat` runs this as merged, in the merged tree, with the two folders in
NSTACK_BASE_PATH and NSTACK_HEAD_PATH (.nstack/handbook/03-contracts.md §3). It compares the
schemas a version holds — its `*.schema.json` files — and nothing else: a contract's pages
are read by a person.

The rule is the consumers': everything a producer may write under the changed schema must
be something the merged schema already allowed. A field that becomes optional, is renamed
or changes type is refused; one that is added, where the merged schema left room for it, is
not. Unchanged structure with a changed meaning is seen by no comparison: contract tests
see it.
"""

import os
import sys
from pathlib import Path

import jsoncompat


def schemas(folder):
    return {path.name: path.read_text(encoding="utf-8") for path in sorted(Path(folder).glob("*.schema.json"))}


def main():
    name = f"{os.environ.get('NSTACK_CONTRACT', 'contract')} {os.environ.get('NSTACK_VERSION', '')}".strip()
    merged, changed = schemas(os.environ["NSTACK_BASE_PATH"]), schemas(os.environ["NSTACK_HEAD_PATH"])
    refused = []
    for schema in sorted(merged.keys() | changed.keys()):
        if schema not in changed:
            refused.append(f"{schema}: removed — what it promised is promised by nothing")
        elif schema not in merged:
            print(f"{name}: {schema} is new — no schema was merged for it, nothing to compare")
        else:
            found = jsoncompat.analyze_compat(merged[schema], changed[schema], "serializer")
            if found["status"] == "compatible":
                print(f"{name}: {schema} compatible")
            else:
                example = found.get("counterexample_json", "none given")
                refused.append(
                    f"{schema}: {found['status']} — the changed schema allows what the merged one refuses: {example}"
                )
    if not merged and not changed:
        print(f"{name}: no schema in this version — its pages are not compared")
    for line in refused:
        print(f"{name}: {line}")
    return 1 if refused else 0


if __name__ == "__main__":
    sys.exit(main())
