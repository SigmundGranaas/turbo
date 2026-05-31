#!/usr/bin/env python3
"""Per-hike score delta between two terrain_metrics runs.

Usage: python3 tools/eval_diff.py <baseline_dir> <variant_dir>

Prints the aggregate delta plus the hikes that moved most in each
direction — so a config change's *distribution* of effect is visible,
not just the average (a knob that helps 7 over-climbers but hurts 33
good routes nets negative even if it's "right" for the 7).
"""
import json
import sys
from pathlib import Path


def load(d):
    out = {}
    for f in Path(d).glob("*.json"):
        if f.name == "_summary.json":
            continue
        r = json.loads(f.read_text())
        out[r["name"]] = r
    return out


def main():
    base = load(sys.argv[1])
    var = load(sys.argv[2])
    common = sorted(set(base) & set(var))
    deltas = []
    for n in common:
        d = var[n]["score"] - base[n]["score"]
        deltas.append((d, n, base[n]["score"], var[n]["score"]))
    avg_b = sum(base[n]["score"] for n in common) / len(common)
    avg_v = sum(var[n]["score"] for n in common) / len(common)
    improved = [x for x in deltas if x[0] > 0.5]
    regressed = [x for x in deltas if x[0] < -0.5]
    print(f"hikes={len(common)}  avg {avg_b:.1f} -> {avg_v:.1f}  ({avg_v-avg_b:+.1f})")
    print(f"improved>0.5: {len(improved)}   regressed<-0.5: {len(regressed)}   flat: {len(common)-len(improved)-len(regressed)}")
    deltas.sort()
    print("\nMost REGRESSED:")
    for d, n, b, v in deltas[:8]:
        print(f"  {n:<22} {b:5.1f} -> {v:5.1f}  ({d:+.1f})")
    print("\nMost IMPROVED:")
    for d, n, b, v in deltas[-8:][::-1]:
        print(f"  {n:<22} {b:5.1f} -> {v:5.1f}  ({d:+.1f})")


if __name__ == "__main__":
    main()
