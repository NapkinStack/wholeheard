"""Every schema a contract holds: that it is one, that the comparator decides its own
harmless changes and refuses its breaks, and that it says what its page says."""

import copy
import json
import sys
from pathlib import Path

import jsonschema
import pytest

CONTRACTS = Path(__file__).parent.parent
sys.path.insert(0, str(CONTRACTS / "scripts"))
import compat

SCHEMAS = sorted(CONTRACTS.glob("*/v*/*.schema.json"))
NAMES = [path.relative_to(CONTRACTS).as_posix() for path in SCHEMAS]


def objects(schema, at=()):
    """Every object a schema describes, with where it is: each kind of a union, and the objects inside."""
    if isinstance(schema, dict):
        if schema.get("type") == "object" and "properties" in schema:
            yield at
        for key, value in schema.items():
            yield from objects(value, (*at, key))
    elif isinstance(schema, list):
        for number, value in enumerate(schema):
            yield from objects(value, (*at, number))


def places(schema, wanted, at=()):
    """Where in a schema something is said that `wanted` recognises."""
    if isinstance(schema, dict):
        if wanted(schema):
            yield at
        for key, value in schema.items():
            yield from places(value, wanted, (*at, key))
    elif isinstance(schema, list):
        for number, value in enumerate(schema):
            yield from places(value, wanted, (*at, number))


def within(schema, at):
    for step in at:
        schema = schema[step]
    return schema


def verdict(tmp_path, merged, changed):
    """What the module's comparator says of a change: nothing when it is compatible."""
    for name, schema in (("merged", merged), ("changed", changed)):
        (tmp_path / f"{name}.schema.json").write_text(json.dumps(schema))
    return compat.judged(tmp_path / "merged.schema.json", tmp_path / "changed.schema.json")


def tells(schema, at):
    """Whether an object is told from the others of its union by a constant, as the rule asks."""
    return any("const" in within(schema, at)["properties"][name] for name in within(schema, at).get("required", []))


@pytest.mark.parametrize("path", SCHEMAS, ids=NAMES)
def test_a_schema_is_one_and_the_comparator_decides_its_harmless_changes(path, tmp_path):
    schema = json.loads(path.read_text())
    jsonschema.Draft202012Validator.check_schema(schema)
    assert verdict(tmp_path, schema, schema) is None
    harmless = {}
    for at in objects(schema):
        added, required = copy.deepcopy(schema), copy.deepcopy(schema)
        within(added, at)["properties"]["added_later"] = {"type": "string"}
        within(required, at)["properties"]["added_later"] = {"type": "number"}
        within(required, at).setdefault("required", []).append("added_later")
        harmless |= {f"a field added at {at}": added, f"a field added and required at {at}": required}
    for name, values in schema.get("$defs", {}).items():
        if len(values.get("enum", [])) > 1:
            fewer = copy.deepcopy(schema)
            fewer["$defs"][name]["enum"].pop()
            harmless[f"a value of {name} no longer written"] = fewer
    for number in range(len(schema.get("oneOf", [])) - 1):
        fewer = copy.deepcopy(schema)
        fewer["oneOf"].pop(number)
        harmless[f"kind {number} no longer written"] = fewer
    # Where a schema leaves room for kinds it does not name, a kind added is one of those, named.
    for number, kind in enumerate(schema.get("oneOf", [])):
        for name, said in kind["properties"].items():
            if "enum" in said.get("not", {}):
                more = copy.deepcopy(schema)
                more["oneOf"][number]["properties"][name]["not"]["enum"].append("added later")
                more["oneOf"].insert(
                    0, {"type": "object", "properties": {name: {"const": "added later"}}, "required": [name]}
                )
                harmless["a kind added"] = more
    refused = {what: why for what, changed in harmless.items() if (why := verdict(tmp_path, schema, changed))}
    assert len(harmless) > 5 and not refused, refused


@pytest.mark.parametrize("path", SCHEMAS, ids=NAMES)
def test_the_comparator_refuses_a_schemas_breaks_wherever_they_are(path, tmp_path):
    schema = json.loads(path.read_text())
    breaks = {}
    for at in objects(schema):
        for name in [
            name
            for name in within(schema, at).get("required", [])
            if "const" not in within(schema, at)["properties"][name]
        ]:
            optional, retyped = copy.deepcopy(schema), copy.deepcopy(schema)
            within(optional, at)["required"].remove(name)
            within(retyped, at)["properties"][name] = {"type": "boolean"}
            breaks |= {f"{name} no longer required at {at}": optional, f"{name} of another type at {at}": retyped}
        for name, said in within(schema, at)["properties"].items():
            if "const" in said:
                loosened = copy.deepcopy(schema)
                within(loosened, at)["properties"][name] = {"type": "string"}
                breaks[f"{name} no longer fixed at {at}"] = loosened
    for name, values in schema.get("$defs", {}).items():
        wider = copy.deepcopy(schema)
        wider["$defs"][name] = {"enum": [*values["enum"], "another"]} if "enum" in values else {}
        breaks[f"{name} allowed more"] = wider
    # A bound taken away, one at a time, and a list of types allowed one more.
    for at in places(schema, lambda said: isinstance(said, dict) and "properties" not in said):
        for bound in ("minimum", "minLength", "not"):
            if bound in within(schema, at):
                unbounded = copy.deepcopy(schema)
                within(unbounded, at).pop(bound)
                breaks[f"{bound} removed at {at}"] = unbounded
        if isinstance(within(schema, at).get("type"), list):
            wider = copy.deepcopy(schema)
            within(wider, at)["type"] = [*within(schema, at)["type"], "boolean"]
            breaks[f"a type allowed more at {at}"] = wider
    breaks = {what: changed for what, changed in breaks.items() if changed != schema}
    let_through = [what for what, changed in breaks.items() if verdict(tmp_path, schema, changed) is None]
    assert len(breaks) > 5 and not let_through, let_through


EXAMPLES = {
    "transcript/v1/line.schema.json": (
        [
            {
                "type": "header",
                "contract": "transcript/v1",
                "engine": "whisper.cpp",
                "model": "large-v3-turbo",
                "language": "fr",
                "context": "Quatre personnes.",
            },
            {
                "type": "header",
                "contract": "transcript/v1",
                "engine": "whisper.cpp",
                "model": "large-v3-turbo",
                "language": None,
                "context": None,
            },
            {
                "type": "segment",
                "speaker": "others",
                "start": 0.0,
                "end": 3.2,
                "text": "Du coup j'ai besoin que l'on confirme la date.",
            },
            {"type": "covered", "speaker": "me", "until": 30.0},
            {"type": "complete", "duration": 1912.4},
            {"type": "a type of a later version", "anything": 1},
        ],
        [
            {"type": "segment", "speaker": "room", "start": 0.0, "end": 3.2, "text": "x"},
            {"type": "segment", "speaker": "me", "start": "0.0", "end": 3.2, "text": "x"},
            {"type": "segment", "speaker": "me", "start": -1, "end": 3.2, "text": "x"},
            {"type": "segment", "speaker": "me", "start": 0.0, "end": -1, "text": "x"},
            {"type": "segment", "speaker": "me", "start": 0.0, "end": 3.2, "text": "on two\nlines"},
            {"type": "segment", "speaker": "me", "start": 0.0, "end": 3.2, "text": ""},
            {"type": "segment", "speaker": "me", "start": 0.0, "end": 3.2, "text": "ends on a line feed\n"},
            {"type": "segment", "speaker": "me", "start": 0.0, "end": 3.2, "text": "\n"},
            {"type": "segment", "speaker": "me", "start": 0.0, "end": 3.2, "text": "a carriage\rreturn"},
            {"type": "segment", "speaker": "me", "start": 0.0, "end": 3.2, "text": None},
            {"type": "covered", "speaker": "anyone", "until": 30.0},
            {"type": "complete", "duration": -1},
            {
                "type": "header",
                "contract": "transcript/v2",
                "engine": "e",
                "model": "m",
                "language": None,
                "context": None,
            },
            {
                "type": "header",
                "contract": "transcript/v1",
                "engine": 7,
                "model": "m",
                "language": None,
                "context": None,
            },
            {"speaker": "me", "start": 0.0, "end": 3.2, "text": "x"},
            "a line that is no object",
        ],
    ),
    "recording/v1/event.schema.json": (
        [
            {"event": "unavailable", "source": "me", "reason": "the system names no default device"},
            {"event": "unavailable", "source": "others", "reason": "the system names no default output"},
            {
                "event": "started",
                "folder": "<sessions-folder>/2026-10-13_14-05",
                "sources": {"me": "unavailable", "others": "Speaker"},
            },
            {"event": "silent", "source": "others"},
            {"event": "heard", "source": "others"},
            {"event": "stopped", "folder": "<sessions-folder>/2026-10-13_14-05", "seconds": 1912.4},
            {"event": "an event of a later version"},
        ],
        [
            {"event": "silent", "source": "local"},
            {"event": "heard", "source": "room"},
            {"event": "unavailable", "source": "anyone", "reason": "r"},
            {"event": "started", "folder": "f", "sources": {"others": "Speaker"}},
            "a line that is no object",
            {"event": "silent"},
            {"event": "started", "folder": "f", "sources": {"me": "Digital Microphone"}},
            {"event": "started", "sources": {"me": "a", "others": "b"}},
            {"event": "stopped", "folder": "f", "seconds": "1912.4"},
            {"event": "stopped", "folder": "f", "seconds": -1},
            {"source": "me"},
        ],
    ),
}


@pytest.mark.parametrize("path", SCHEMAS, ids=NAMES)
def test_a_schema_allows_what_its_page_shows_and_refuses_what_its_page_refuses(path):
    allowed, refused = EXAMPLES[path.relative_to(CONTRACTS).as_posix()]
    reads = jsonschema.Draft202012Validator(json.loads(path.read_text()))
    assert [line for line in allowed if not reads.is_valid(line)] == []
    assert [line for line in refused if reads.is_valid(line)] == []
    # Every field an example of a named kind carries is one its kind requires — the examples
    # carry no other: without any one of them, the line is refused.
    schema = json.loads(path.read_text())
    for kind in schema["oneOf"]:
        fixed = {name: said["const"] for name, said in kind["properties"].items() if "const" in said}
        lines = [line for line in allowed if fixed and isinstance(line, dict) and fixed.items() <= line.items()]
        assert not fixed or lines, f"no example of {fixed}"
        for line in lines:
            kept = [name for name in line if reads.is_valid({key: value for key, value in line.items() if key != name})]
            assert kept == [], f"{fixed}: allowed without {kept}"
            nullable = {name for other in lines for name, value in other.items() if value is None}
            nulled = [name for name in line if name not in nullable and reads.is_valid({**line, name: None})]
            assert nulled == [], f"{fixed}: allowed with {nulled} null"
    # The page's own examples — its lines of JSON — are lines the schema allows.
    page = (path.parent / "README.md").read_text()
    shown = [json.loads(line) for line in page.splitlines() if line.startswith("{") and line.endswith("}")]
    assert len(shown) >= 2 and [line for line in shown if not reads.is_valid(line)] == []
