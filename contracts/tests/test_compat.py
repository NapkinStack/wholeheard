"""The comparator, on versions made here: what it lets through and what it refuses."""

import json
import os
import subprocess
import sys
from pathlib import Path

SCRIPT = Path(__file__).parent.parent / "scripts" / "compat.py"
SEGMENT = {
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "type": "object",
    "required": ["type", "start", "end", "text"],
    "properties": {
        "type": {"enum": ["segment", "complete"]},
        "start": {"type": "number"},
        "end": {"type": "number"},
        "text": {"type": "string"},
    },
}


def changed(change):
    schema = json.loads(json.dumps(SEGMENT))
    change(schema)
    return schema


def judged(tmp_path, merged, new, page=("a page", "a page")):
    """Runs the comparator as `nstack compat` does: two folders, each a version of a contract."""
    for folder, files, text in (("base", merged, page[0]), ("head", new, page[1])):
        (tmp_path / folder).mkdir(parents=True)
        (tmp_path / folder / "README.md").write_text(text)
        for name, schema in files.items():
            (tmp_path / folder / name).write_text(json.dumps(schema))
    env = {**os.environ, "NSTACK_CONTRACT": "transcript", "NSTACK_VERSION": "v1"}
    env |= {"NSTACK_BASE_PATH": str(tmp_path / "base"), "NSTACK_HEAD_PATH": str(tmp_path / "head")}
    run = subprocess.run([sys.executable, str(SCRIPT)], check=False, env=env, capture_output=True, text=True)
    return run.returncode, run.stdout + run.stderr


def test_what_no_consumer_can_notice_is_compatible(tmp_path):
    cases = {
        "the same schema": lambda schema: None,
        "a field added": lambda schema: schema["properties"].update(confidence={"type": "number"}),
        "a field that was free now required": lambda schema: schema["required"].append("speaker"),
        "a value no longer written": lambda schema: schema["properties"].update(type={"enum": ["segment"]}),
    }
    for number, (what, change) in enumerate(cases.items()):
        versions = {"line.schema.json": SEGMENT}, {"line.schema.json": changed(change)}
        code, said = judged(tmp_path / str(number), *versions)
        assert code == 0 and "transcript v1: line.schema.json compatible" in said, f"{what}: {said}"


def test_what_a_consumer_relied_on_is_refused_with_an_example(tmp_path):
    cases = {
        "a required field no longer required": lambda schema: schema["required"].remove("end"),
        "a field renamed": lambda schema: (schema["required"].remove("start"), schema["required"].append("begin")),
        "a type changed": lambda schema: schema["properties"].update(start={"type": "string"}),
        "a value added": lambda schema: schema["properties"].update(type={"enum": ["segment", "complete", "pause"]}),
    }
    for number, (what, change) in enumerate(cases.items()):
        versions = {"line.schema.json": SEGMENT}, {"line.schema.json": changed(change)}
        code, said = judged(tmp_path / str(number), *versions)
        assert code == 1 and "the changed schema allows what the merged one refuses: {" in said, f"{what}: {said}"


def test_a_schema_is_never_removed_and_a_first_schema_is_free(tmp_path):
    code, said = judged(tmp_path / "removed", {"line.schema.json": SEGMENT}, {})
    assert code == 1 and "line.schema.json: removed" in said, said
    code, said = judged(tmp_path / "first", {}, {"line.schema.json": SEGMENT})
    assert code == 0 and "line.schema.json is new" in said, said
    # One refused among several refuses the version, and each is said.
    both = {"line.schema.json": SEGMENT, "header.schema.json": SEGMENT}
    one = {"line.schema.json": changed(lambda schema: schema["required"].remove("end")), "header.schema.json": SEGMENT}
    code, said = judged(tmp_path / "several", both, one)
    assert code == 1 and "header.schema.json compatible" in said and "line.schema.json: incompatible" in said, said


def test_pages_are_not_compared(tmp_path):
    code, said = judged(tmp_path / "pages", {}, {}, page=("it promised this", "it promises that"))
    assert code == 0 and "no schema in this version — its pages are not compared" in said, said
    same = {"line.schema.json": SEGMENT}
    code, said = judged(tmp_path / "with", same, same, page=("one", "another"))
    assert code == 0, said
