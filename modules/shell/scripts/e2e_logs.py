"""What the e2e's runs logged, read against what its scenarios provoke (D29): every warning
and error of Wholeheard's logs found in the run's folder must be one the manifest's
`evidence.provoked` declares, at its level — any other fails the run, quoted; one declared and
never seen fails it too, its scenario no longer provoking it. The logs are kept in the run's
evidence, where NapkinStack reads them again ($NSTACK_EVIDENCE)."""
import json
import pathlib
import re
import shutil
import sys

import yaml

work, evidence, manifest = (pathlib.Path(arg) for arg in sys.argv[1:4])
kept = evidence / "logs"
shutil.rmtree(kept, ignore_errors=True)
kept.mkdir(parents=True)
declared = yaml.safe_load(manifest.read_text())["evidence"]["provoked"]
patterns = [(item, re.compile(item["pattern"])) for item in declared]
levels = {"WARN": "warning", "ERROR": "error"}
seen, unknown, lines = set(), {}, 0
for number, log in enumerate(sorted(work.glob("**/logs/*.jsonl"))):
    shutil.copy(log, kept / f"{number:02d}-{log.name}")
    for line in log.read_text(errors="replace").splitlines():
        try:
            entry = json.loads(line)
        except ValueError:
            # A line cut by a program killed: a failure when it says one.
            if "WARN" in line or "ERROR" in line:
                unknown.setdefault(line, line)
            continue
        lines += 1
        level = levels.get(entry.get("level"))
        if level is None:
            continue
        hit = [item["pattern"] for item, pattern in patterns if item["level"] == level and pattern.search(line)]
        seen.update(hit)
        if not hit:
            unknown.setdefault(entry.get("message", line), line)
unseen = [item for item in declared if item["pattern"] not in seen]
print(f"    {lines} log lines kept in {kept}; {len(unknown)} warning or error message(s) not provoked, "
      f"{len(unseen)} declared and never logged")
for line in unknown.values():
    print(f"    not provoked by any scenario: {line}")
for item in unseen:
    print(f"    declared and never logged: {item['pattern']} ({item['level']}, scenario {item['scenario']})")
sys.exit(1 if unknown or unseen else 0)
