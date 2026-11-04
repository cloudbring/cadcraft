#!/usr/bin/env python3
"""Turn docs/verification/data/*.json into the tables used by docs/verification/solver-speedup.md.

Speedup = old/new = 1 / (craft-bench ratio); its CI is [1/ci_hi, 1/ci_lo].
"""
import glob, json, math, os, re, sys

D = os.path.join(os.path.dirname(__file__), "../../docs/verification/data")


def load(arm):
    rows = {}
    pub = {}
    for f in sorted(glob.glob(f"{D}/solver-{arm}-*.json")):
        r = json.load(open(f))
        pub[os.path.basename(f)] = (r["publishable"], r["contended"], r["guard"]["load_before"], r["guard"]["load_after"], r["config"]["runs_per_side"])
        for i in r["inputs"]:
            rows[i["name"]] = i
    agree = {r["name"]: r for r in json.load(open(f"{D}/agreement-{arm}.json"))}
    return rows, agree, pub


def sp(m):
    r = m["ratio"]
    return 1 / r["point"], 1 / r["ci_hi"], 1 / r["ci_lo"]


def fmt(x):
    if x >= 100:
        return f"{x:,.0f}"
    if x >= 10:
        return f"{x:.1f}"
    return f"{x:.2f}"


def ms(ns):
    return f"{ns / 1e6:.3f}"


def cell(i, clock="wall"):
    m = i[clock]
    if m is None:
        return "n/a"
    p, lo, hi = sp(m)
    return f"{fmt(p)}x [{fmt(lo)}, {fmt(hi)}]"


def verdict(i):
    m = i["wall"]["ratio"]
    if m["ci_lo"] > 1:
        return "slower"
    if m["ci_hi"] < 1:
        return "faster"
    return "inconclusive"


def main():
    out = []
    for arm in ("uncapped", "main"):
        try:
            rows, agree, pub = load(arm)
        except FileNotFoundError:
            continue
        out.append(f"## arm {arm}\n")
        out.append("Files: " + "; ".join(f"{k}: publishable={v[0]} contended={v[1]} load {v[2]}->{v[3]} runs/side={v[4]}" for k, v in pub.items()) + "\n")
        syn = [n for n in rows if n.startswith("syn-")]
        shapes = ["chain", "grid", "mix", "clustered"]
        sizes = [50, 200, 500, 1000, 2000, 5000]
        out.append("### synthetic speedup (old median / new median, wall; bootstrap 95% CI)\n")
        out.append("| constraints | " + " | ".join(shapes) + " |")
        out.append("|---:|" + "---|" * len(shapes))
        for s in sizes:
            cells = []
            for sh in shapes:
                n = f"syn-{sh}-{s:04d}"
                if n in rows:
                    cells.append(cell(rows[n]))
                elif n in agree:
                    cells.append(f"not timed ({agree[n]['verdict']}: old {agree[n]['old_result']})")
                else:
                    cells.append("-")
            out.append(f"| {s} | " + " | ".join(cells) + " |")
        out.append("")
        out.append("### synthetic absolute medians (ms) old / new, and CPU-time speedup\n")
        out.append("| input | constraints | old ms | new ms | wall speedup | CPU speedup | clone us | batch |")
        out.append("|---|---:|---:|---:|---|---|---:|---:|")
        for n in sorted(syn):
            if n in rows:
                i = rows[n]
                a = agree[n]
                out.append(f"| {n} | {a['constraints']} | {ms(i['wall']['old']['median_ns'])} | {ms(i['wall']['new']['median_ns'])} | {cell(i)} | {cell(i, 'cpu')} | {a['clone_us']:.1f} | {a['batch']} |")
        out.append("")
        real = [n for n in rows if not n.startswith("syn-")]
        out.append(f"### real sketches: {len(real)} timed\n")
        for var in ("@exact", "@jitter"):
            names = [n for n in real if n.endswith(var)]
            v = {"faster": 0, "slower": 0, "inconclusive": 0}
            for n in names:
                v[verdict(rows[n])] += 1
            sps = [sp(rows[n]["wall"])[0] for n in names]
            gm = math.exp(sum(math.log(x) for x in sps) / len(sps)) if sps else float("nan")
            out.append(f"- {var}: {len(names)} inputs; faster {v['faster']}, slower {v['slower']}, inconclusive {v['inconclusive']}; geometric-mean speedup {gm:.2f}x")
        out.append("")
        bands = [(0, 49), (50, 199), (200, 499), (500, 1999), (2000, 99999)]
        out.append("| real inputs by constraint count | @exact: n, geo-mean speedup, min..max | @jitter: n, geo-mean speedup, min..max |")
        out.append("|---|---|---|")
        for lo, hi in bands:
            cs = []
            for var in ("@exact", "@jitter"):
                v = [sp(rows[n]["wall"])[0] for n in real if n.endswith(var) and lo <= agree[n]["constraints"] <= hi]
                cs.append(f"{len(v)}, {math.exp(sum(map(math.log, v)) / len(v)):.2f}x, {min(v):.2f}..{max(v):.2f}x" if v else "-")
            out.append(f"| {lo}-{hi if hi < 99999 else 'up'} | {cs[0]} | {cs[1]} |")
        out.append("")
        out.append("| real input | constraints | old ms | new ms | wall speedup [CI] | verdict |")
        out.append("|---|---:|---:|---:|---|---|")
        for n in sorted(real, key=lambda n: -agree[n]["constraints"]):
            i = rows[n]
            out.append(f"| {n} | {agree[n]['constraints']} | {ms(i['wall']['old']['median_ns'])} | {ms(i['wall']['new']['median_ns'])} | {cell(i)} | {verdict(i)} |")
        out.append("")
        out.append("### agreement\n")
        vs = {}
        for a in agree.values():
            vs[a["verdict"]] = vs.get(a["verdict"], 0) + 1
        out.append(f"verdicts: {vs}\n")
        diffs = [a["max_param_diff"] for a in agree.values() if a["max_param_diff"] is not None]
        diffs.sort()
        if diffs:
            out.append(f"max parameter difference over {len(diffs)} comparable inputs: median {diffs[len(diffs)//2]:.2e}, p90 {diffs[int(len(diffs)*.9)]:.2e}, max {diffs[-1]:.2e}\n")
        out.append(f"residual after solve (max over constraints): old max {max(a['old_residual_post'] for a in agree.values() if a['old_result']=='ok'):.2e}, new max {max(a['new_residual_post'] for a in agree.values() if a['new_result']=='ok'):.2e}\n")
        out.append(f"DOF/rank text equal pre-solve: {sum(a['dof_pre_equal'] for a in agree.values())}/{len(agree)}; post-solve: {sum(a['dof_post_equal'] for a in agree.values())}/{len(agree)}\n")
        out.append(f"constraints skipped (uninterpretable): old {sum(a['old_skipped'] for a in agree.values())}, new {sum(a['new_skipped'] for a in agree.values())}\n")
        out.append("non-agreeing inputs:\n")
        for a in agree.values():
            if a["verdict"] != "agree":
                out.append(f"- {a['name']} ({a['constraints']} cons): {a['verdict']}; old {a['old_result']!r}, new {a['new_result']!r}; max diff {a['max_param_diff']}; post-residual old {a['old_residual_post']:.2e} new {a['new_residual_post']:.2e}; dof post old `{a['old_dof_post']}` new `{a['new_dof_post']}`")
        out.append("")
    print("\n".join(out))


main()
