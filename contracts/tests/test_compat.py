"""The comparator, on versions made here: what it lets through and what it refuses."""

import json
import os
import subprocess
import sys
from pathlib import Path

SCRIPT = Path(__file__).parent.parent / "scripts" / "compat.py"
SPEAKER = {"$ref": "#/$defs/speaker"}
# Lines of one file, each of one kind, as a contract of this project describes them: every
# kind says it is an object and requires what tells it from the others.
LINES = {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
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
    ],
    "$defs": {"speaker": {"enum": ["me", "others"]}},
}


def changed(change, schema=LINES):
    schema = json.loads(json.dumps(schema))
    change(schema)
    return schema


def judged(tmp_path, merged, new, env=None):
    """Runs the comparator as `nstack compat` does: two folders, each a version of a contract."""
    for folder, files in (("base", merged), ("head", new)):
        (tmp_path / folder).mkdir(parents=True)
        (tmp_path / folder / "README.md").write_text(f"the page, at the {folder}")
        for name, schema in files.items():
            (tmp_path / folder / name).parent.mkdir(parents=True, exist_ok=True)
            (tmp_path / folder / name).write_text(schema if isinstance(schema, str) else json.dumps(schema))
    paths = {"NSTACK_BASE_PATH": str(tmp_path / "base"), "NSTACK_HEAD_PATH": str(tmp_path / "head")}
    env = {**os.environ, "NSTACK_CONTRACT": "transcript", "NSTACK_VERSION": "v1", **paths, **(env or {})}
    run = subprocess.run([sys.executable, str(SCRIPT)], check=False, env=env, capture_output=True, text=True)
    return run.returncode, run.stdout + run.stderr


def segment(schema):
    return schema["oneOf"][0]


def test_what_no_consumer_can_notice_is_compatible(tmp_path):
    cases = {
        "the same schema": lambda schema: None,
        "a field added": lambda schema: segment(schema)["properties"].update(confidence={"type": "number"}),
        "a field added and required": lambda schema: (
            segment(schema)["properties"].update(end={"type": "number"}),
            segment(schema)["required"].append("end"),
        ),
        "a value no longer written": lambda schema: schema["$defs"].update(speaker={"enum": ["me"]}),
        "a kind of line no longer written": lambda schema: schema["oneOf"].pop(),
    }
    for number, (what, change) in enumerate(cases.items()):
        code, said = judged(tmp_path / str(number), {"line.schema.json": LINES}, {"line.schema.json": changed(change)})
        assert code == 0 and "transcript v1: line.schema.json compatible — pages are not compared" in said, (
            f"{what}: {said}"
        )


def test_what_a_consumer_relied_on_is_refused_with_what_it_would_now_receive(tmp_path):
    cases = {
        "a required field no longer required": (
            lambda schema: segment(schema)["required"].remove("start"),
            '"speaker"',
        ),
        "a field renamed": (
            lambda schema: (
                segment(schema)["properties"].update(begin=segment(schema)["properties"].pop("start")),
                segment(schema)["required"].remove("start"),
                segment(schema)["required"].append("begin"),
            ),
            '"begin"',
        ),
        "a type changed": (lambda schema: segment(schema)["properties"].update(start={"type": "string"}), '"start":"'),
        "a value added": (lambda schema: schema["$defs"].update(speaker={"enum": ["me", "others", "room"]}), '"room"'),
    }
    for number, (what, (change, received)) in enumerate(cases.items()):
        code, said = judged(tmp_path / str(number), {"line.schema.json": LINES}, {"line.schema.json": changed(change)})
        example = said.partition("the changed schema allows what the merged one refuses: ")[2]
        assert code == 1 and received in example and json.loads(example), f"{what}: {said}"


def test_what_cannot_be_decided_or_read_is_refused_and_said(tmp_path):
    # A change the comparator cannot decide — a pattern, here made stricter — is refused with its reason.
    words = {"type": "string", "pattern": "^[a-z]+$"}
    code, said = judged(
        tmp_path / "undecided", {"word.schema.json": words}, {"word.schema.json": {**words, "pattern": "^[a-c]+$"}}
    )
    assert (
        code == 1
        and "word.schema.json: the comparison could not be decided, and what is not proven is refused (" in said
    ), said
    # What it cannot read is refused by name, and the other schemas of the version are still judged.
    older = {"$schema": "http://json-schema.org/draft-07/schema#", "type": "object"}
    unread = {"old.schema.json": older, "text.schema.json": "not json", "line.schema.json": LINES}
    code, said = judged(tmp_path / "unread", unread, unread)
    for expected in (
        "old.schema.json: it cannot be compared (",
        "text.schema.json: it cannot be compared (",
        "line.schema.json compatible",
    ):
        assert code == 1 and expected in said and "Traceback" not in said, said
    code, said = judged(tmp_path / "unset", {}, {}, env={"NSTACK_HEAD_PATH": ""})
    assert code == 1 and "are not set" in said and "Traceback" not in said, said


def test_a_schema_is_never_removed_and_a_first_schema_must_read(tmp_path):
    code, said = judged(tmp_path / "removed", {"line.schema.json": LINES}, {})
    assert code == 1 and "line.schema.json: removed" in said, said
    code, said = judged(tmp_path / "first", {}, {"line.schema.json": LINES})
    assert code == 0 and "line.schema.json is new, and reads" in said, said
    # A first schema the comparator cannot read would freeze its version: it is refused at once.
    code, said = judged(tmp_path / "unreadable", {}, {"line.schema.json": {"$ref": "other.schema.json#/$defs/line"}})
    assert code == 1 and "line.schema.json: it cannot be compared (" in said, said
    # One refused among several refuses the version, and each is said.
    broken = changed(lambda schema: segment(schema)["required"].remove("start"))
    code, said = judged(
        tmp_path / "several",
        {"a.schema.json": LINES, "b.schema.json": LINES},
        {"a.schema.json": broken, "b.schema.json": LINES},
    )
    assert code == 1 and "b.schema.json compatible" in said and "a.schema.json: the changed schema allows" in said, said


def test_a_version_with_no_schema_and_a_schema_where_none_is_read_are_refused(tmp_path):
    code, said = judged(tmp_path / "none", {}, {})
    assert code == 1 and "no schema in this version: nothing can be compared" in said, said
    for number, stray in enumerate(("schemas/line.schema.json", "line.json", "line.schema.yaml")):
        code, said = judged(
            tmp_path / str(number), {"line.schema.json": LINES}, {"line.schema.json": LINES, stray: LINES}
        )
        assert code == 1 and f"{stray}: a schema is a `*.schema.json` file at the version's top" in said, said
