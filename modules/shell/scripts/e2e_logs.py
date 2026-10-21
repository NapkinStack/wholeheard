"""What the e2e's runs logged, read against what its scenarios provoke (D29): every warning
and error of Wholeheard's logs found in the run's folder must be one `e2e-logged.txt` names,
with the scenario that provokes it — any other fails the run, quoted; one named and never seen
fails it too, its scenario no longer provoking it. The logs are kept in the evidence."""
import json
import pathlib
import shutil
import sys

work, evidence, expected = (pathlib.Path(arg) for arg in sys.argv[1:4])
kept = evidence / "logs"
shutil.rmtree(kept, ignore_errors=True)
kept.mkdir(parents=True)
seen, lines, cut = {}, 0, []
for number, log in enumerate(sorted(work.glob("**/logs/*.jsonl"))):
    shutil.copy(log, kept / f"{number:02d}-{log.name}")
    for line in log.read_text(errors="replace").splitlines():
        try:
            entry = json.loads(line)
        except ValueError:
            # A line cut by a program killed: kept apart, and a failure when it says one.
            if "WARN" in line or "ERROR" in line:
                cut.append(line)
            continue
        lines += 1
        if entry.get("level") in ("WARN", "ERROR"):
            seen.setdefault(entry.get("message", "?"), line)
named = {}
for row in expected.read_text().splitlines():
    if row.strip() and not row.startswith("#"):
        message, _, why = row.partition(" ")
        named[message] = why.strip()
unknown = sorted(set(seen) - set(named)) + cut
unseen = sorted(set(named) - set(seen))
print(f"    {lines} log lines kept in {kept}; {len(seen)} warning or error message(s); "
      f"{len(unknown)} not provoked, {len(set(named) - set(seen))} named and never logged")
for message in unknown:
    print(f"    not provoked by any scenario: {seen.get(message, message)}")
for message in unseen:
    print(f"    named and never logged: {message} ({named[message]})")
sys.exit(1 if unknown or unseen else 0)
