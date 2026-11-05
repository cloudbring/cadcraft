"""Dump DXF files through ezdxf as canonical JSON: the oracle side of tools/dxf-oracle.

    python -I dump_ezdxf.py OUT_DIR ROOT REL_PATH [REL_PATH ...]
    python -I dump_ezdxf.py --audit-only OUT_DIR ROOT REL_PATH ...   (no entity dump; audit result only)

Each ROOT/REL_PATH becomes OUT_DIR/<REL_PATH with / replaced by __>.json. The schema is shared
with the Rust side (crates/io/examples/dxf_oracle/dump.rs); see tools/dxf-oracle/README.md.

Canonical conventions (both sides): angles in degrees except ELLIPSE parameters (radians); points
are [x, y, z]; handles are upper-case hex without padding; colours are an ACI int (0 ByBlock, 256
ByLayer) or "rgb:RRGGBB"; lineweights are the DXF integers (-1 ByLayer, -2 ByBlock, -3 Default).
Nothing here is rounded: tolerances belong to the comparison.
"""

import json
import math
import os
import re
import sys

import ezdxf
from ezdxf import recover


_U = re.compile(r"\\U\+([0-9A-Fa-f]{4})")


def u(s):
    """Decode the \\U+XXXX escapes DXF uses for non-ASCII characters (ezdxf keeps them as written)."""
    return _U.sub(lambda m: chr(int(m.group(1), 16)), s) if isinstance(s, str) else s


def pt(v):
    if v is None:
        return None
    return [float(v[0]), float(v[1]), float(v[2]) if len(v) > 2 else 0.0]


def num(x):
    x = float(x)
    return x if math.isfinite(x) else str(x)


def color_of(e):
    if e.dxf.hasattr("true_color"):
        return "rgb:%06X" % (int(e.dxf.true_color) & 0xFFFFFF)
    return int(e.dxf.get("color", 256))


def norm(v):
    n = math.sqrt(sum(c * c for c in v))
    return [c / n for c in v] if n > 0 else list(v)


def geom_line(e):
    return {"a": pt(e.dxf.start), "b": pt(e.dxf.end)}


def geom_point(e):
    return {"p": pt(e.dxf.location)}


def geom_circle(e):
    return {"center": pt(e.dxf.center), "radius": num(e.dxf.radius)}


def geom_arc(e):
    return {
        "center": pt(e.dxf.center),
        "radius": num(e.dxf.radius),
        "start_deg": num(e.dxf.start_angle),
        "end_deg": num(e.dxf.end_angle),
    }


def geom_ellipse(e):
    return {
        "center": pt(e.dxf.center),
        "major": pt(e.dxf.major_axis),
        "ratio": num(e.dxf.ratio),
        "start_param": num(e.dxf.start_param),
        "end_param": num(e.dxf.end_param),
    }


def geom_lwpolyline(e):
    return {
        "vertices": [
            {"p": [float(x), float(y)], "bulge": float(b), "sw": float(s), "ew": float(w)}
            for x, y, s, w, b in e.get_points("xyseb")
        ],
        "closed": bool(e.closed),
        "const_width": num(e.dxf.get("const_width", 0.0)),
    }


def geom_polyline2d(e):
    # The old-style 2D POLYLINE is canonicalised to LWPOLYLINE.
    return {
        "vertices": [
            {
                "p": [float(v.dxf.location[0]), float(v.dxf.location[1])],
                "bulge": float(v.dxf.get("bulge", 0.0)),
                "sw": float(v.dxf.get("start_width", 0.0)),
                "ew": float(v.dxf.get("end_width", 0.0)),
            }
            for v in e.vertices
        ],
        "closed": bool(e.is_closed),
        "const_width": 0.0,
    }


def geom_polyline3d(e):
    return {"points": [pt(v.dxf.location) for v in e.vertices], "closed": bool(e.is_closed)}


def geom_polyface(e):
    verts = list(e.vertices)
    coords = [v for v in verts if not v.is_face_record]
    faces = [v for v in verts if v.is_face_record]
    return {"vertices": [pt(v.dxf.location) for v in coords], "face_count": len(faces)}


def geom_spline(e):
    return {
        "degree": int(e.dxf.degree),
        "control": [pt(p) for p in e.control_points],
        "knots": [float(k) for k in e.knots],
        "weights": [float(w) for w in e.weights],
        "fit": [pt(p) for p in e.fit_points],
        "closed": bool(e.closed),
    }


def geom_ray(e):
    return {"base": pt(e.dxf.start), "dir": norm(pt(e.dxf.unit_vector))}


def geom_text(e):
    # The point the text is placed by: group 10 for left/baseline, aligned and fit text, group 11
    # for justified text (group 10 of justified text is a recomputed, advisory start point).
    h, v = int(e.dxf.get("halign", 0)), int(e.dxf.get("valign", 0))
    anchor = e.dxf.insert if (h, v) == (0, 0) or h in (3, 5) or not e.dxf.hasattr("align_point") else e.dxf.align_point
    g = {
        "text": u(e.dxf.text),
        "height": num(e.dxf.height),
        "anchor": pt(anchor),
        "rotation_deg": num(e.dxf.get("rotation", 0.0)),
        "width_factor": num(e.dxf.get("width", 1.0)),
        "oblique_deg": num(e.dxf.get("oblique", 0.0)),
        "style": u(e.dxf.get("style", "Standard")),
        "halign": int(e.dxf.get("halign", 0)),
        "valign": int(e.dxf.get("valign", 0)),
    }
    return g


def geom_mtext(e):
    rot = e.dxf.get("rotation", 0.0)
    if e.dxf.hasattr("text_direction"):
        d = e.dxf.text_direction
        rot = math.degrees(math.atan2(d[1], d[0]))
    return {
        "text": u(e.text),
        "height": num(e.dxf.get("char_height", 0.0)),
        "insert": pt(e.dxf.insert),
        "width": num(e.dxf.get("width", 0.0)),
        "attach": int(e.dxf.get("attachment_point", 1)),
        "rotation_deg": num(rot),
        "style": u(e.dxf.get("style", "Standard")),
    }


def geom_attdef(e):
    g = geom_text(e)
    g["tag"] = u(e.dxf.tag)
    return g


def geom_insert(e):
    return {
        "block": u(e.dxf.name),
        "insert": pt(e.dxf.insert),
        "scale": [num(e.dxf.get("xscale", 1.0)), num(e.dxf.get("yscale", 1.0)), num(e.dxf.get("zscale", 1.0))],
        "rotation_deg": num(e.dxf.get("rotation", 0.0)),
        "cols": int(e.dxf.get("column_count", 1)),
        "rows": int(e.dxf.get("row_count", 1)),
        "col_spacing": num(e.dxf.get("column_spacing", 0.0)),
        "row_spacing": num(e.dxf.get("row_spacing", 0.0)),
        "attribs": [{"tag": u(a.dxf.tag), "text": u(a.dxf.text)} for a in e.attribs],
    }


DIM_POINTS = [
    ("defpt", "defpoint"),
    ("text_mid", "text_midpoint"),
    ("p13", "defpoint2"),
    ("p14", "defpoint3"),
    ("p15", "defpoint4"),
    ("p16", "defpoint5"),
]


def geom_dimension(e):
    g = {"dimtype": int(e.dxf.get("dimtype", 0)) & 7, "style": e.dxf.get("dimstyle", "Standard")}
    if g["dimtype"] == 0:
        g["rotation_deg"] = num(e.dxf.get("angle", 0.0))
    for key, attr in DIM_POINTS:
        if e.dxf.hasattr(attr):
            g[key] = pt(e.dxf.get(attr))
    if e.dxf.hasattr("actual_measurement"):
        g["measurement"] = num(e.dxf.actual_measurement)
    g["text"] = u(e.dxf.get("text", ""))
    return g


def geom_hatch(e):
    return {
        "pattern": e.dxf.get("pattern_name", ""),
        "solid": bool(e.dxf.get("solid_fill", 0)),
        "loops": len(e.paths),
        "associative": bool(e.dxf.get("associative", 0)),
    }


def geom_solid(e):
    return {"corners": [pt(e.dxf.get("vtx%d" % i, None)) for i in range(4)]}


def geom_leader(e):
    return {
        "vertices": [pt(v) for v in e.vertices],
        "arrow": bool(e.dxf.get("has_arrowhead", 0)),
        "spline": int(e.dxf.get("path_type", 0)) == 1,
    }


GEOM = {
    "LINE": geom_line,
    "POINT": geom_point,
    "CIRCLE": geom_circle,
    "ARC": geom_arc,
    "ELLIPSE": geom_ellipse,
    "LWPOLYLINE": geom_lwpolyline,
    "SPLINE": geom_spline,
    "RAY": geom_ray,
    "XLINE": geom_ray,
    "TEXT": geom_text,
    "MTEXT": geom_mtext,
    "ATTDEF": geom_attdef,
    "INSERT": geom_insert,
    "DIMENSION": geom_dimension,
    "HATCH": geom_hatch,
    "SOLID": geom_solid,
    "TRACE": geom_solid,
    "3DFACE": geom_solid,
    "LEADER": geom_leader,
}


def canonical_type(e):
    t = e.dxftype()
    if t == "POLYLINE":
        if e.is_poly_face_mesh:
            return "POLYFACE", geom_polyface
        if e.is_polygon_mesh:
            return "POLYMESH", None
        if e.is_3d_polyline:
            return "POLYLINE3D", geom_polyline3d
        return "LWPOLYLINE", geom_polyline2d
    return t, GEOM.get(t)


def entity_record(e, space):
    t, fn = canonical_type(e)
    rec = {
        "space": space,
        "type": t,
        "handle": e.dxf.handle.upper() if e.dxf.hasattr("handle") else None,
        "layer": u(e.dxf.get("layer", "0")),
        "color": color_of(e),
        "linetype": e.dxf.get("linetype", "BYLAYER"),
        "lineweight": int(e.dxf.get("lineweight", -1)),
        "geom": {},
    }
    if e.dxf.hasattr("extrusion"):
        x = e.dxf.extrusion
        if (round(x[0], 9), round(x[1], 9), round(x[2], 9)) != (0.0, 0.0, 1.0):
            # Object coordinates in a flipped system: the comparison skips this entity's geometry.
            rec["ocs"] = pt(x)
    if fn is not None:
        try:
            rec["geom"] = fn(e)
        except Exception as ex:  # a malformed entity must not hide the rest of the file
            rec["geom_error"] = "%s: %s" % (type(ex).__name__, ex)
    return rec


def layer_record(la):
    if la.dxf.hasattr("true_color"):
        color = "rgb:%06X" % (int(la.dxf.true_color) & 0xFFFFFF)
    else:
        color = abs(int(la.dxf.get("color", 7)))
    return {
        "color": color,
        "linetype": la.dxf.get("linetype", "Continuous"),
        "lineweight": int(la.dxf.get("lineweight", -3)),
        "on": not la.is_off(),
        "frozen": la.is_frozen(),
        "locked": la.is_locked(),
        "plot": bool(la.dxf.get("plot", 1)),
    }


def dump_doc(doc):
    out = {"version": getattr(doc, "loaded_dxfversion", None) or doc.dxfversion, "insunits": int(doc.header.get("$INSUNITS", 0))}
    out["layers"] = {u(la.dxf.name): layer_record(la) for la in doc.layers}
    entities, blocks, paper = [], {}, []
    for layout in doc.layouts:
        space = "model" if layout.name == "Model" else "paper:" + layout.name
        if space != "model":
            paper.append(layout.name)
        for e in layout:
            entities.append(entity_record(e, space))
    for blk in doc.blocks:
        if blk.is_any_layout:
            continue
        counts = {}
        for e in blk:
            rec = entity_record(e, "block:" + u(blk.name))
            entities.append(rec)
            counts[rec["type"]] = counts.get(rec["type"], 0) + 1
        blocks[u(blk.name)] = {"base": pt(blk.block.dxf.base_point), "count": sum(counts.values()), "types": counts}
    out["blocks"] = blocks
    out["paper_layouts"] = sorted(paper)
    out["entities"] = entities
    return out


def audit_doc(doc):
    auditor = doc.audit()
    return {
        "errors": [{"code": int(x.code), "message": x.message} for x in auditor.errors],
        "fixes": len(auditor.fixes),
    }


def process(path, audit_only):
    res = {"file": path, "tool": "ezdxf " + ezdxf.__version__, "ok": False, "recovered": False, "error": None}
    doc = None
    try:
        doc = ezdxf.readfile(path)
        res["ok"] = True
    except Exception as ex:
        res["error"] = "%s: %s" % (type(ex).__name__, ex)
        try:
            doc, _auditor = recover.readfile(path)
            res["recovered"] = True
        except Exception as ex2:
            res["recover_error"] = "%s: %s" % (type(ex2).__name__, ex2)
    if doc is not None:
        try:
            if not audit_only:
                res.update(dump_doc(doc))
            res["audit"] = audit_doc(doc)
        except Exception as ex:
            res["dump_error"] = "%s: %s" % (type(ex).__name__, ex)
    return res


def main(argv):
    audit_only = False
    if argv and argv[0] == "--audit-only":
        audit_only, argv = True, argv[1:]
    if len(argv) < 3:
        sys.stderr.write(__doc__)
        return 2
    out_dir, root, rels = argv[0], argv[1], argv[2:]
    os.makedirs(out_dir, exist_ok=True)
    for rel in rels:
        res = process(os.path.join(root, rel), audit_only)
        res["file"] = rel
        name = rel.replace("/", "__") + ".json"
        with open(os.path.join(out_dir, name), "w") as f:
            json.dump(res, f, sort_keys=True, indent=1)
            f.write("\n")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
