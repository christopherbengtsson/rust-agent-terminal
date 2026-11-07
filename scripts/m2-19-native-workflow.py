#!/usr/bin/env python3
"""Prepare/run the ticket-19 native workflow through real keyboard plans.

Run one phase at a time and inspect its evidence before continuing. This driver
never approves a native tool command. Trust confirmation applies only to the
exact disposable workspace observed in its current native screen. Native tool
approvals/questions must be answered in a separate explicitly inspected plan.
"""
import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import time

DRIVER = Path(__file__).with_name("m2-19-native.py")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("phase", choices=["register", "create", "controls", "editors", "archive"])
    parser.add_argument("--root", type=Path, required=True)
    parser.add_argument("--source", type=Path, default=DRIVER.parent.parent)
    parser.add_argument("--binary", type=Path)
    parser.add_argument("--workspace", default="amber-claude")
    parser.add_argument("--parser", type=Path)
    args = parser.parse_args()
    args.binary = args.binary or args.source / "target/debug/agent-terminal"
    projects = json.loads((args.root / "projects.json").read_text())
    repo = Path(projects[0]["path"])
    evidence = args.root / "evidence"
    evidence.mkdir(exist_ok=True)
    plans = args.root / "plans"
    plans.mkdir(exist_ok=True)
    pin = args.root / "pin.json"

    def command(action, *extra):
        return [sys.executable, str(DRIVER), action, *(["--parser", str(args.parser)] if args.parser else []), "--root", str(args.root),
                "--source", str(args.source), "--binary", str(args.binary),
                "--pin", str(pin), *extra]

    def observe():
        label = "state-" + str(time.time_ns())
        subprocess.run(command("observe", "--label", label), check=True,
                       stdout=subprocess.DEVNULL)
        return json.loads((evidence / (label + ".json")).read_text())

    def drive(label, steps, **options):
        label = str(time.time_ns()) + "-" + label
        plan = {"geometry": [40, 160], "steps": steps, **options}
        path = plans / (label + ".json")
        path.write_text(json.dumps(plan, indent=2) + "\n")
        subprocess.run(command("drive", "--repo", str(repo), "--plan", str(path),
                               "--label", label), check=True)
        return json.loads((evidence / label / "result.json").read_text())["observation"]

    def text(value, wait=.3):
        return {"text": value, "wait": wait}

    def key(value, wait=.5):
        return {"hex": value, "wait": wait}

    def select(workspace):
        value = observe()
        active = [row for row in value["registry"]["workspaces"]
                  if row["id"] not in value["registry"]["archived"]]
        index = next(i for i, row in enumerate(active) if row["id"] == workspace["id"])
        return [text("w"), text("k" * (len(active)+1)), text("j" * index), key("0d", 1)]

    def workspace(name):
        path = os.fsencode(str(args.root / name))
        value = observe()
        return next(row for row in value["registry"]["workspaces"]
                    if bytes(row.get("path") or []) == path)

    if args.phase == "register":
        steps = [text("a", 1)]
        for project in projects:
            steps += [text("A"), text(project["path"]), key("0d", 1)]
        drive("register-three-projects", steps)
    elif args.phase == "create":
        for project in projects:
            base = workspace(project["name"])
            drive("setup-"+project["name"], select(base) + [text("U"), key("15"),
                  text("printf 'm2-19 setup complete\\n'; printf 'setup saved\\n' >> proof.txt"),
                  key("0d", 1)])
            for agent in ["claude", "codex"]:
                name = project["name"] + "-" + agent
                folder = args.root / name
                marker = "M2_19_" + name.upper().replace("-", "_")
                prompt = "Reply exactly " + marker + ". Do not use tools or change files."
                steps = select(base) + [text("N", 1), key("15"), text("main"), key("09"),
                      key("15"), text("workspace/m2-19-"+name), key("09"), key("15"),
                      text(str(folder)), key("09"), text(prompt), key("09")]
                if agent == "codex":
                    steps += [key("1b5b43")]
                steps += [key("13", 4), {"capture": "created-and-started", "wait": .5}]
                value = drive("create-"+name, steps)
                created = workspace(name)
                value = drive("attach-"+name, select(created)+[key("0d", 3),
                              {"capture": "first-native-frame", "wait": .5}])
                native = value.get("screen", "")
                # Confirm only the freshly observed native folder-trust prompt.
                # This does not accept native command or hook permissions.
                if agent == "claude" and "Quick safety check:" in native and str(folder) in native:
                    drive("trust-"+name, select(created)+[key("0d"), key("1b5b42"), key("0d", 7)])
                elif agent == "codex" and "Trust this folder?" in native and str(folder) in native:
                    drive("trust-"+name, select(created)+[key("0d"), key("0d", 7)])
                value = observe()
                if value.get("lifecycle") != "Running":
                    raise RuntimeError("native not running; inspect " + name)
        final = observe()
        sessions = [row for row in final["sessions"] if row.get("lifecycle") == "Running"]
        (args.root / "six-native-sessions.json").write_text(json.dumps(final, indent=2)+"\n")
        # Session field names vary only by version; do not silently relabel a
        # failed or starting process as six running sessions.
        if len(sessions) != 6:
            raise RuntimeError("expected six actual Running summaries; inspect retained evidence")
    elif args.phase == "controls":
        for agent in ["claude", "codex"]:
            current = workspace("amber-"+agent)
            steps = select(current)+[key("0d"),
                    {"paste": "Ask me one question before proceeding. Do not use tools or change files.", "wait": .2},
                    key("0d", 15), {"capture": "native-question", "wait": .1}]
            drive("question-"+agent, steps)
            drive("resize-"+agent, select(current)+[key("0d"),
                  {"resize": [18, 60], "wait": 1, "capture": "native-60x18"},
                  {"resize": [40, 160], "wait": 1, "capture": "native-160x40"},
                  key("1d"), {"capture": "dashboard-focus", "wait": .5},
                  key("0d"), {"capture": "same-session-attach", "wait": 1}])
    elif args.phase == "editors":
        current = workspace(args.workspace)
        for name, config, close in [
            ("vim", {"executable":"/usr/bin/vim", "arguments":["-u","NONE"], "position":"vim"}, "1b3a710d"),
            ("pico", {"executable":"/usr/bin/pico", "arguments":[], "position":"plus"}, "18")]:
            steps = select(current)+[text("f", 2), text("e", 2),
                  {"capture": name+"-working", "wait": .5}, key(close, 1), text("w", 2),
                  text("e", 2), {"capture": name+"-whole", "wait": .5}, key(close, 1)]
            drive("comparison-editors-"+name, steps, editor=config)
            # Open the known tracked live source through the selected diff file.
            # Setup changed proof.txt, so Working inventory has that source.
            edits = select(current)+[text("f", 2), text("o", 2)]
            if name == "vim":
                edits += [key("1b"), text("GoM2_19_VIM_SOURCE_SAVED"), key("1b"), text(":wq"), key("0d", 1)]
            else:
                edits += [key("05"), key("0d"), text("M2_19_PICO_SOURCE_SAVED"), key("0f"), key("0d"), key("18", 1)]
            drive("live-source-"+name, edits, editor=config)
    elif args.phase == "archive":
        current = workspace(args.workspace)
        drive("stop-save-archive-"+args.workspace, select(current)+[text("K", 1),
              {"capture":"archive-preview", "wait":.2}, key("0d", 5),
              {"capture":"archive-branch-result", "wait":1}])
        # This phase deliberately stops before Return/removal. Inspect the exact
        # durable archive/branch result and recovery ref before those actions.


if __name__ == "__main__":
    main()
