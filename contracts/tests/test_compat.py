"""The comparator, on versions made here: what it lets through and what it refuses."""

import json
import os
import subprocess
import sys
from pathlib import Path

SCRIPT = Path(__file__).parent.parent / "scripts" / "compat.py"
DIALECT = "https://json-schema.org/draft/2020-12/schema"
SPEAKER = {"$ref": "#/$defs/speaker"}
MADE = {"type": "object", "properties": {"engine": {"type": "string"}}, "required": ["engine"]}
# Lines of one file, each of one kind, written as the module's rule says: a `oneOf`, every
# kind an object that requires what tells it from the others, a shared field by its own `$ref`.
LINES = {
    "$schema": DIALECT,
    "oneOf": [
        {
            "type": "object",
            "properties": {"type": {"const": "segment"}, "speaker": SPEAKER, "start": {"type": "number"}},
            "required": ["type", "speaker", "start"],
        },
        {
            "type": "object",
            "properties": {"type": {"const": "covered"}, "speaker": SPEAKER, "until": {"type": "number"}},
            "required": ["type", "speaker", "until"],
        },
        {
            "type": "object",
            "properties": {"type": {"const": "header"}, "made": MADE},
            "required": ["type", "made"],
        },
    ],
    "$defs": {"speaker": {"enum": ["me", "others"]}},
}


def changed(change):
    schema = json.loads(json.dumps(LINES))
    change(schema)
    return schema


def judged(tmp_path, merged, new, env=None):
    """Runs the comparator as `nstack compat` does: two folders, each a version of a contract."""
    for folder, files in (("base", merged), ("head", new)):
        (tmp_path / folder).mkdir(parents=True)
        (tmp_path / folder / "README.md").write_text(f"the page, at the {folder}")
        for name, schema in files.items():
            (tmp_path / folder / name).parent.mkdir(parents=True, exist_ok=True)
            content = (
                schema
                if isinstance(schema, bytes)
                else (schema if isinstance(schema, str) else json.dumps(schema)).encode()
            )
            (tmp_path / folder / name).write_bytes(content)
    paths = {"NSTACK_BASE_PATH": str(tmp_path / "base"), "NSTACK_HEAD_PATH": str(tmp_path / "head")}
    env = {**os.environ, "NSTACK_CONTRACT": "transcript", "NSTACK_VERSION": "v1", **paths, **(env or {})}
    run = subprocess.run([sys.executable, str(SCRIPT)], check=False, env=env, capture_output=True, text=True)
    assert "Traceback" not in run.stdout + run.stderr, run.stderr
    return run.returncode, run.stdout + run.stderr


def kind(schema, number):
    return schema["oneOf"][number]


def test_what_no_consumer_can_notice_is_compatible(tmp_path):
    cases = {
        "the same schema": lambda schema: None,
        "a field added": lambda schema: kind(schema, 1)["properties"].update(confidence={"type": "number"}),
        "a field added and required": lambda schema: (
            kind(schema, 0)["properties"].update(end={"type": "number"}),
            kind(schema, 0)["required"].append("end"),
        ),
        "a field added inside an object": lambda schema: kind(schema, 2)["properties"]["made"]["properties"].update(
            model={"type": "string"}
        ),
        "a value no longer written": lambda schema: schema["$defs"].update(speaker={"enum": ["me"]}),
        "a kind of line no longer written": lambda schema: schema["oneOf"].pop(1),
    }
    for number, (what, change) in enumerate(cases.items()):
        code, said = judged(tmp_path / str(number), {"line.schema.json": LINES}, {"line.schema.json": changed(change)})
        assert code == 0 and "transcript v1: line.schema.json compatible — pages are not compared" in said, (
            f"{what}: {said}"
        )


def test_what_a_consumer_relied_on_is_refused_with_what_it_would_now_receive(tmp_path):
    cases = {
        "a required field no longer required": (
            lambda schema: kind(schema, 0)["required"].remove("start"),
            '"segment"',
        ),
        "a field renamed, in the second kind": (
            lambda schema: (
                kind(schema, 1)["properties"].update(to=kind(schema, 1)["properties"].pop("until")),
                kind(schema, 1)["required"].remove("until"),
                kind(schema, 1)["required"].append("to"),
            ),
            '"to"',
        ),
        "a type changed, in the second kind": (
            lambda schema: kind(schema, 1)["properties"].update(until={"type": "string"}),
            '"until":"',
        ),
        "a required field no longer required, inside an object": (
            lambda schema: kind(schema, 2)["properties"]["made"].pop("required"),
            '"made":{',
        ),
        "a value added": (lambda schema: schema["$defs"].update(speaker={"enum": ["me", "others", "room"]}), '"room"'),
    }
    for number, (what, (change, received)) in enumerate(cases.items()):
        code, said = judged(tmp_path / str(number), {"line.schema.json": LINES}, {"line.schema.json": changed(change)})
        # The comparator looks for an example and does not always find one: a break is
        # refused either way, with the example or with its reason.
        example = said.partition("line.schema.json: the changed schema allows what the merged one refuses: ")[2]
        reason = said.partition(
            "line.schema.json: the comparison could not be decided, and what is not proven is refused ("
        )[2]
        assert code == 1 and (received in example and json.loads(example) if example else len(reason) > 20), (
            f"{what}: {said}"
        )


def test_what_cannot_be_decided_or_read_is_refused_and_said(tmp_path):
    # A change the comparator cannot decide — a pattern, here made stricter — is refused with its reason.
    words = {"$schema": DIALECT, "type": "string", "pattern": "^[a-z]+$"}
    code, said = judged(
        tmp_path / "undecided", {"word.schema.json": words}, {"word.schema.json": {**words, "pattern": "^[a-c]+$"}}
    )
    reason = said.partition(
        "word.schema.json: the comparison could not be decided, and what is not proven is refused ("
    )[2]
    assert code == 1 and "pattern" in reason, said
    # What it cannot read is refused by name with why, and the other schemas of the version are still judged.
    unread = {
        "older.schema.json": {"$schema": "http://json-schema.org/draft-07/schema#", "type": "object"},
        "text.schema.json": "not json",
        "bytes.schema.json": b"\xff\xfe\x00{",
        "elsewhere.schema.json": {"$schema": DIALECT, "$ref": "other.schema.json#/$defs/line"},
        "line.schema.json": LINES,
    }
    code, said = judged(tmp_path / "unread", unread, unread)
    for name in ("older", "text", "bytes"):
        assert f"{name}.schema.json: it does not say it is a schema of {DIALECT}" in said, said
    assert (
        code == 1
        and "elsewhere.schema.json: it cannot be compared (" in said
        and "$ref" in said
        and "line.schema.json compatible" in said
    ), said
    for unset in ("NSTACK_BASE_PATH", "NSTACK_HEAD_PATH"):
        code, said = judged(tmp_path / unset, {}, {}, env={unset: ""})
        assert code == 1 and "are not set" in said, said


def test_a_schema_is_never_removed_and_a_first_schema_must_read(tmp_path):
    code, said = judged(tmp_path / "removed", {"line.schema.json": LINES}, {})
    assert code == 1 and "line.schema.json: removed" in said, said
    code, said = judged(tmp_path / "first", {}, {"line.schema.json": LINES})
    assert code == 0 and "line.schema.json is new, and reads" in said, said
    # A first schema that says nothing, or that the comparator cannot read, is refused at once:
    # merged, it would open its version's pages, or freeze the version for good.
    for number, first in enumerate(({}, True, {"$schema": DIALECT, "$ref": "other.schema.json#/$defs/line"})):
        code, said = judged(tmp_path / f"first-{number}", {}, {"line.schema.json": first})
        assert code == 1 and "line.schema.json: it " in said, said
    # One refused among several refuses the version, and each is said.
    broken = changed(lambda schema: kind(schema, 1)["required"].remove("until"))
    code, said = judged(
        tmp_path / "several",
        {"a.schema.json": LINES, "b.schema.json": LINES},
        {"a.schema.json": broken, "b.schema.json": LINES},
    )
    assert code == 1 and "b.schema.json compatible" in said and "a.schema.json: the c" in said, said


def test_a_version_with_no_schema_and_a_schema_where_none_is_read_are_refused(tmp_path):
    code, said = judged(tmp_path / "none", {}, {})
    assert code == 1 and "no schema in this version: nothing can be compared" in said, said
    for number, stray in enumerate(("schemas/line.schema.json", "line.schema.yaml", "line.json", "schemas/other.json")):
        code, said = judged(
            tmp_path / str(number), {"line.schema.json": LINES}, {"line.schema.json": LINES, stray: LINES}
        )
        assert code == 1 and f"{stray}: a schema is a `*.schema.json` file at the version's top" in said, said
    # An example is not a schema: a document, or lines, beside the schema they illustrate.
    examples = {
        "examples/valid.json": {"type": "segment", "speaker": "me", "start": 3.4},
        "examples/lines.jsonl": '{"type":"covered"}\n',
    }
    code, said = judged(tmp_path / "examples", {"line.schema.json": LINES}, {"line.schema.json": LINES, **examples})
    assert code == 0, said
