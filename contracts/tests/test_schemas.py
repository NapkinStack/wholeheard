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
    # Never vacuous: a flat object, as minutes/v1's front matter, gives its two — a field added,
    # added and required —; a schema of unions and enums, many more.
    assert len(harmless) >= 2 and not refused, refused


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
    for at in places(schema, lambda said: isinstance(said, dict) and "oneOf" not in said):
        for bound in ("minimum", "minLength", "minItems", "minProperties", "items", "anyOf", "not"):
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


# The fields a page says may be absent: `stopped`, while a session is recording; a segment's
# `track` and `voice`, in a transcript written before they were; a `silent`'s `nothing`, said by
# a capture that knows it (#361).
OPTIONAL = {"stopped", "track", "voice", "nothing"}
SHOWN_AS_LINES = {
    "transcript/v1/line.schema.json",
    "recording/v1/event.schema.json",
    "summarising/v1/event.schema.json",
}
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
            {
                "type": "segment",
                "speaker": "me",
                "track": "local",
                "voice": None,
                "start": 3.4,
                "end": 5.1,
                "text": "Première semaine de juin, je pense.",
            },
            {
                "type": "segment",
                "speaker": "others",
                "track": "remote",
                "voice": "2",
                "start": 6.0,
                "end": 7.0,
                "text": "x",
            },
            {"type": "segment", "speaker": "me", "start": 7.0, "end": 8.0, "text": "Written before the track."},
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
            {"type": "segment", "speaker": "me", "track": "room", "start": 0.0, "end": 3.2, "text": "x"},
            {"type": "segment", "speaker": "me", "track": None, "start": 0.0, "end": 3.2, "text": "x"},
            {"type": "segment", "speaker": "me", "voice": "", "start": 0.0, "end": 3.2, "text": "x"},
            {"type": "segment", "speaker": "me", "voice": "two\nlines", "start": 0.0, "end": 3.2, "text": "x"},
            {"type": "segment", "speaker": "me", "voice": 2, "start": 0.0, "end": 3.2, "text": "x"},
            {"type": "segment", "speaker": "me", "track": "both", "start": 0.0, "end": 3.2, "text": "x"},
            {"type": "segment", "speaker": "me", "track": "remote", "start": 0.0, "end": 3.2, "text": "x"},
            {"type": "segment", "speaker": "others", "track": "local", "start": 0.0, "end": 3.2, "text": "x"},
            {"type": "segment", "speaker": "me", "voice": True, "start": 0.0, "end": 3.2, "text": "x"},
            {"type": "segment", "speaker": "me", "voice": ["1"], "start": 0.0, "end": 3.2, "text": "x"},
            {"type": "segment", "speaker": "me", "voice": "a\rb", "start": 0.0, "end": 3.2, "text": "x"},
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
            {"event": "silent", "source": "me", "nothing": True},
            {"event": "silent", "source": "others", "nothing": False},
            {"event": "heard", "source": "others"},
            {"event": "level", "source": "me", "level": 0.42},
            {"event": "device", "source": "me", "device": "Headset Microphone"},
            {"event": "device", "source": "others", "device": None},
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
            {"event": "silent", "source": "me", "nothing": "yes"},
            {"event": "started", "folder": "f", "sources": {"me": "Digital Microphone"}},
            {"event": "started", "sources": {"me": "a", "others": "b"}},
            {"event": "stopped", "folder": "f", "seconds": "1912.4"},
            {"event": "stopped", "folder": "f", "seconds": -1},
            {"event": "level", "source": "me"},
            {"event": "level", "source": "me", "level": 1.5},
            {"event": "level", "source": "me", "level": -0.1},
            {"event": "level", "source": "room", "level": 0.5},
            {"event": "level", "level": 0.5},
            {"event": "device", "source": "me"},
            {"event": "device", "device": "Headset Microphone"},
            {"event": "device", "source": "room", "device": "Headset Microphone"},
            {"event": "device", "source": "me", "device": 7},
            {"source": "me"},
        ],
    ),
    "summarising/v1/event.schema.json": (
        [
            {"event": "sending", "host": "openrouter.ai", "model": "anthropic/claude-sonnet-5.5", "characters": 48210},
            {"event": "sending", "host": "local", "model": "mistral", "characters": 31},
            {"event": "written", "file": "<session-folder>/minutes.md"},
            {"event": "answered", "host": "local", "model": "mistral"},
            {"event": "failed", "code": "provider.refused_key"},
            {"event": "failed", "code": "settings.invalid"},
            {"event": "failed", "code": "a code of a later version"},
            {"event": "an event of a later version"},
        ],
        [
            "a line that is no object",
            {"host": "openrouter.ai"},
            {"event": "sending", "host": "openrouter.ai", "model": "m"},
            {"event": "sending", "host": "", "model": "m", "characters": 1},
            {"event": "sending", "host": "h", "model": "", "characters": 1},
            {"event": "answered", "host": "h", "model": ""},
            {"event": "sending", "host": "h", "model": "m", "characters": 1.5},
            {"event": "sending", "host": "h", "model": "m", "characters": -1},
            {"event": "written"},
            {"event": "written", "file": ""},
            {"event": "answered", "host": "h"},
            {"event": "answered", "model": "m"},
            {"event": "failed"},
            {"event": "failed", "code": ""},
            {"event": "failed", "code": "provider.refused_key\n"},
            {"event": "failed", "code": 401},
        ],
    ),
    "session-audio/v1/session.schema.json": (
        [
            {
                "contract": "session-audio/v1",
                "started": "2026-10-13T14:05:12+02:00",
                "sources": {"me": {"device": "Digital Microphone"}, "others": {"device": "Speaker"}},
            },
            {
                "contract": "session-audio/v1",
                "started": "2026-10-13T14:05:12+02:00",
                "stopped": "2026-10-13T14:37:04+02:00",
                "sources": {"me": {"device": "unavailable"}, "others": {"device": "Speaker"}},
            },
        ],
        [
            {
                "contract": "session-audio/v2",
                "started": "2026-10-13T14:05:12+02:00",
                "sources": {"me": {"device": "a"}, "others": {"device": "b"}},
            },
            {
                "contract": "session-audio/v1",
                "started": "2026-10-13T14:05:12+02:00\n",
                "sources": {"me": {"device": "a"}, "others": {"device": "b"}},
            },
            {
                "contract": "session-audio/v1",
                "started": "",
                "sources": {"me": {"device": "a"}, "others": {"device": "b"}},
            },
            {
                "contract": "session-audio/v1",
                "started": "2026-10-13T14:05:12+02:00",
                "sources": {"me": {"device": "a"}},
            },
            {
                "contract": "session-audio/v1",
                "started": "2026-10-13T14:05:12+02:00",
                "sources": {"me": {}, "others": {"device": "b"}},
            },
            {
                "contract": "session-audio/v1",
                "started": "2026-10-13T14:05:12+02:00",
                "stopped": 1,
                "sources": {"me": {"device": "a"}, "others": {"device": "b"}},
            },
            {
                "contract": "session-audio/v1",
                "started": "2026-10-13T14:05:12\r+02:00",
                "sources": {"me": {"device": "a"}, "others": {"device": "b"}},
            },
            {
                "contract": "session-audio/v1",
                "started": "2026-10-13T14:05:12+02:00",
                "sources": {"others": {"device": "b"}},
            },
            {
                "contract": "session-audio/v1",
                "started": "2026-10-13T14:05:12+02:00",
                "sources": {"me": {"device": None}, "others": {"device": "b"}},
            },
            "a record that is no object",
        ],
    ),
    "session-audio/v2/session.schema.json": (
        [
            {
                "contract": "session-audio/v2",
                "started": "2026-10-14T14:05:12+02:00",
                "tracks": {"local": {"device": "Digital Microphone"}, "remote": {"device": "Speaker"}},
            },
            {
                "contract": "session-audio/v2",
                "started": "2026-10-14T14:05:12+02:00",
                "stopped": "2026-10-14T15:05:12+02:00",
                "tracks": {"remote": {"device": "Speaker"}},
            },
            {
                "contract": "session-audio/v2",
                "started": "2026-10-14T14:05:12+02:00",
                "tracks": {
                    "local": {
                        "device": "Digital Microphone",
                        "changed": [
                            {"at": 312.5, "device": "Headset Microphone"},
                            {"at": 1290.0, "device": None},
                            {"at": 1302.5, "device": "Digital Microphone"},
                        ],
                    },
                    "remote": {"device": "Speaker", "changed": []},
                },
            },
        ],
        [
            {
                "contract": "session-audio/v1",
                "started": "2026-10-14T14:05:12+02:00",
                "tracks": {"remote": {"device": "Speaker"}},
            },
            {"contract": "session-audio/v2", "started": "2026-10-14T14:05:12+02:00", "tracks": {}},
            {
                "contract": "session-audio/v2",
                "started": "2026-10-14T14:05:12+02:00",
                "tracks": {"local": {"name": "Speaker"}},
            },
            {
                "contract": "session-audio/v2",
                "started": "2026-10-14T14:05:12+02:00\n",
                "tracks": {"remote": {"device": "Speaker"}},
            },
            {
                "contract": "session-audio/v2",
                "started": "2026-10-14T14:05:12+02:00",
                "sources": {"me": {"device": "a"}, "others": {"device": "b"}},
            },
            {"contract": "session-audio/v2", "started": "", "tracks": {"remote": {"device": "Speaker"}}},
            {
                "contract": "session-audio/v2",
                "started": "2026-10-14T14:05:12\r+02:00",
                "tracks": {"remote": {"device": "Speaker"}},
            },
            {
                "contract": "session-audio/v2",
                "started": "2026-10-14T14:05:12+02:00",
                "tracks": {"room": {"device": "a mixer"}},
            },
            {
                "contract": "session-audio/v2",
                "started": "2026-10-14T14:05:12+02:00",
                "tracks": {"local": {"device": "a"}, "remote": None},
            },
            {
                "contract": "session-audio/v2",
                "started": "2026-10-14T14:05:12+02:00",
                "tracks": {"remote": {"device": None}},
            },
            {
                "contract": "session-audio/v2",
                "started": "2026-10-14T14:05:12+02:00",
                "tracks": {"local": {"device": "a", "changed": {"at": 1.0, "device": "b"}}},
            },
            {
                "contract": "session-audio/v2",
                "started": "2026-10-14T14:05:12+02:00",
                "tracks": {"local": {"device": "a", "changed": [{"device": "b"}]}},
            },
            {
                "contract": "session-audio/v2",
                "started": "2026-10-14T14:05:12+02:00",
                "tracks": {"local": {"device": "a", "changed": [{"at": "1.0", "device": "b"}]}},
            },
            {
                "contract": "session-audio/v2",
                "started": "2026-10-14T14:05:12+02:00",
                "tracks": {"local": {"device": "a", "changed": [{"at": 1.0}]}},
            },
            {
                "contract": "session-audio/v2",
                "started": "2026-10-14T14:05:12+02:00",
                "tracks": {"local": {"device": "a", "changed": [{"at": 1.0, "device": 7}]}},
            },
            "a record that is no object",
        ],
    ),
    "minutes/v1/front-matter.schema.json": (
        [
            {
                "contract": "minutes/v1",
                "made": "2026-10-13T15:12:40+02:00",
                "provider": "local",
                "model": "mistral",
                "context": None,
            },
            {
                "contract": "minutes/v1",
                "made": "2026-10-28T10:00:00+01:00",
                "provider": "openrouter.ai",
                "model": "anthropic/claude-opus-5.5",
                "context": "Budget du T4 avec Inès Durand.",
            },
        ],
        [
            {
                "contract": "minutes/v2",
                "made": "2026-10-13T15:12:40+02:00",
                "provider": "local",
                "model": "mistral",
                "context": None,
            },
            {"contract": "minutes/v1", "made": "", "provider": "local", "model": "mistral", "context": None},
            {
                "contract": "minutes/v1",
                "made": "2026-10-13T15:12:40+02:00",
                "provider": "",
                "model": "mistral",
                "context": None,
            },
            {
                "contract": "minutes/v1",
                "made": "2026-10-13T15:12:40+02:00",
                "provider": "local",
                "model": "mistral",
                "context": 3,
            },
            {"contract": "minutes/v1", "made": "2026-10-13T15:12:40+02:00", "provider": "local", "model": "mistral"},
            "a front matter that is no object",
        ],
    ),
    "session-audio/v2/clock.schema.json": (
        [
            {"track": "remote", "sample": 0, "at": 0.021},
            {"track": "local", "sample": 480000, "at": 10.0341},
            {"track": "local", "sample": 0, "at": -0.004},
            {"track": "remote", "silence": [96000, 240000]},
            {"track": "local", "silence": [0, 2048]},
        ],
        [
            {"track": "room", "sample": 0, "at": 0.0},
            {"track": "local", "sample": -1, "at": 0.0},
            {"track": "local", "sample": 1.5, "at": 0.0},
            {"track": "local", "sample": 0, "at": "0.0"},
            {"track": "remote", "silence": [96000]},
            {"track": "remote", "silence": [96000, 240000, 3]},
            {"track": "remote", "silence": [-1, 3]},
            {"track": "remote", "silence": [96000, 240000], "sample": 96000, "at": 2.0},
            {"sample": 0, "at": 0.0},
            "a line that is no object",
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
    for kind in schema.get("oneOf", [schema]):
        fixed = {name: said["const"] for name, said in kind["properties"].items() if "const" in said}
        # A kind is known by its constants, or else by the fields it requires; the kind left
        # open for what a later version may add is no kind of its own.
        if any("not" in said for said in kind.get("properties", {}).values()):
            continue
        named = set(kind.get("required", [])) if not fixed else set()
        lines = [
            line
            for line in allowed
            if isinstance(line, dict) and (fixed.items() <= line.items() if fixed else named and named <= set(line))
        ]
        assert not (fixed or named) or lines, f"no example of {fixed or named}"
        for line in lines:
            kept = [
                name
                for name in line
                if name not in OPTIONAL and reads.is_valid({key: value for key, value in line.items() if key != name})
            ]
            assert kept == [], f"{fixed}: allowed without {kept}"
            nullable = {name for other in lines for name, value in other.items() if value is None}
            nulled = [name for name in line if name not in nullable and reads.is_valid({**line, name: None})]
            assert nulled == [], f"{fixed}: allowed with {nulled} null"
    # The page's own examples — its lines of JSON — are lines the schema allows, where the
    # page shows its documents as lines; the others' examples are copied above.
    if path.relative_to(CONTRACTS).as_posix() not in SHOWN_AS_LINES:
        return
    page = (path.parent / "README.md").read_text()
    shown = [json.loads(line) for line in page.splitlines() if line.startswith("{") and line.endswith("}")]
    assert len(shown) >= 2 and [line for line in shown if not reads.is_valid(line)] == []
