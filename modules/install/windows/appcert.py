"""The certification kit's report, said in a few lines (D77).

appcert.exe writes an XML report: REPORT, its OVERALL_RESULT and VERSION, then REQUIREMENT > TEST,
each with NAME, INDEX, OPTIONAL, its MESSAGE TEXTs and its RESULT — as written on GitHub's Windows
runner by the kit 10.0.26100.9457 (tests/appcert-report.xml, cut from it). Run as
`python appcert.py <report.xml>`, it prints them; a report it cannot read is said in one line.
"""
import sys
from xml.etree import ElementTree


def said(path):
    try:
        report = ElementTree.parse(path).getroot()
    except (OSError, ElementTree.ParseError) as error:
        return [f"certification kit: no report read — {error}"]
    if report.tag != "REPORT":
        return [f"certification kit: no report read — {path} holds <{report.tag}>, not the kit's <REPORT>"]
    tests = list(report.iter("TEST"))
    failed = [test for test in tests if (test.findtext("RESULT") or "").strip() != "PASS"]
    lines = [f"certification kit {report.get('VERSION')}: {report.get('OVERALL_RESULT')} overall, "
             f"{len(tests)} tests, {len(failed)} failed"]
    for test in failed:
        result = (test.findtext("RESULT") or "no result").strip()
        required = "optional" if test.get("OPTIONAL") == "TRUE" else "required"
        messages = [message.get("TEXT", "") for message in test.iter("MESSAGE")]
        first = f": {len(messages)} messages, the first: {messages[0]}" if messages else ""
        lines.append(f"  {result}  {test.get('NAME')} (test {test.get('INDEX')}, {required}){first}")
    return lines


if __name__ == "__main__":
    print("\n".join(said(sys.argv[1])))
