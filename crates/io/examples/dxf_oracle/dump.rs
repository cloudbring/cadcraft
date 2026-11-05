//! A drawing as the canonical JSON that `tools/dxf-oracle/dump_ezdxf.py` writes for ezdxf.
//!
//! Conventions (shared with the Python side): angles in degrees (ELLIPSE parameters in radians),
//! points `[x, y, z]`, handles upper-case hex, colours an ACI int or `"rgb:RRGGBB"`, lineweights
//! the DXF integers. Nothing is rounded.

use cadcraft_color::Color;
use cadcraft_doc::{Drawing, Entity, EntityKind, Layer};
use cadcraft_geom::{PolyVertex, Vec2, Vec3};
use serde_json::{Map, Value, json};

fn p3(v: Vec3) -> Value {
    json!([v.x, v.y, v.z])
}

fn p2(v: Vec2) -> Value {
    json!([v.x, v.y, 0.0])
}

fn deg(r: f64) -> f64 {
    r.to_degrees()
}

fn color(c: Color) -> Value {
    match c {
        Color::ByLayer => json!(256),
        Color::ByBlock => json!(0),
        Color::Index(i) => json!(i),
        Color::True(rgb) => json!(format!("rgb:{:06X}", rgb.to_u32())),
    }
}

/// The canonical type name: ezdxf's `dxftype()`, with the 2D POLYLINE folded into LWPOLYLINE.
pub fn canonical_type(k: &EntityKind) -> String {
    match k {
        EntityKind::Polyline3d(_) => "POLYLINE3D".into(),
        EntityKind::PolyfaceMesh(_) => "POLYFACE".into(),
        EntityKind::Dimension(d) if matches!(d.kind, cadcraft_doc::DimKind::ArcLength) => "ARC_DIMENSION".into(),
        other => other.dxf_name().to_string(),
    }
}

fn vertex(v: &PolyVertex) -> Value {
    json!({"p": [v.p.x, v.p.y], "bulge": v.bulge, "sw": v.start_width, "ew": v.end_width})
}

fn text_geom(t: &cadcraft_doc::Text) -> Map<String, Value> {
    let mut g = Map::new();
    g.insert("text".into(), json!(t.value));
    g.insert("height".into(), json!(t.height));
    let j = t.justify_point();
    g.insert("anchor".into(), json!([j.x, j.y, t.insert.z]));
    g.insert("rotation_deg".into(), json!(deg(t.rotation)));
    g.insert("width_factor".into(), json!(t.width_factor));
    g.insert("oblique_deg".into(), json!(deg(t.oblique)));
    g.insert("style".into(), json!(t.style));
    g.insert("halign".into(), json!(t.halign as i32));
    g.insert("valign".into(), json!(t.valign as i32));
    g
}

fn dim_geom(d: &cadcraft_doc::Dimension) -> Value {
    use cadcraft_doc::DimKind as K;
    let (dimtype, rot) = match d.kind {
        K::Linear { rotation } => (0, Some(deg(rotation))),
        K::Aligned => (1, None),
        K::Angular => (2, None),
        K::Diameter => (3, None),
        K::Radius => (4, None),
        K::Angular3P => (5, None),
        K::Ordinate { .. } => (6, None),
        K::ArcLength => (8, None),
    };
    let mut g = json!({
        "dimtype": dimtype, "style": d.style, "defpt": p3(d.defpt), "text_mid": p3(d.text_mid),
        "p13": p3(d.p13), "p14": p3(d.p14), "p15": p3(d.p15), "p16": p3(d.p16),
        "measurement": d.measurement, "text": d.text,
    });
    if let (Some(r), Some(o)) = (rot, g.as_object_mut()) {
        o.insert("rotation_deg".into(), json!(r));
    }
    g
}

fn insert_geom(i: &cadcraft_doc::Insert) -> Value {
    let attribs: Vec<Value> = i.attribs.iter().map(|a| json!({"tag": a.tag, "text": a.text.value})).collect();
    json!({
        "block": i.block, "insert": p3(i.insert), "scale": [i.scale.x, i.scale.y, i.scale.z],
        "rotation_deg": deg(i.rotation), "cols": i.cols, "rows": i.rows,
        "col_spacing": i.col_spacing, "row_spacing": i.row_spacing, "attribs": attribs,
    })
}

fn spline_geom(s: &cadcraft_geom::Spline) -> Value {
    json!({
        "degree": s.degree, "control": s.control.iter().map(|p| p2(*p)).collect::<Vec<_>>(),
        "knots": s.knots, "weights": s.weights, "fit": s.fit.iter().map(|p| p2(*p)).collect::<Vec<_>>(),
        "closed": s.closed,
    })
}

fn unit(v: Vec3) -> Value {
    let n = (v.x * v.x + v.y * v.y + v.z * v.z).sqrt();
    if n > 0.0 { json!([v.x / n, v.y / n, v.z / n]) } else { p3(v) }
}

/// The key geometry of an entity (empty for kinds the oracle does not compare in detail).
pub fn geom(k: &EntityKind) -> Value {
    match k {
        EntityKind::Line(l) => json!({"a": p3(l.a), "b": p3(l.b)}),
        EntityKind::Point(p) => json!({"p": p3(p.p)}),
        EntityKind::Circle(c) => json!({"center": p3(c.center), "radius": c.radius}),
        EntityKind::Arc(a) => json!({"center": p3(a.center), "radius": a.radius, "start_deg": deg(a.start), "end_deg": deg(a.end)}),
        EntityKind::Ellipse(e) => json!({
            "center": p3(e.center), "major": p3(e.major), "ratio": e.ratio, "start_param": e.start, "end_param": e.end,
        }),
        EntityKind::LwPolyline(p) => {
            json!({"vertices": p.vertices.iter().map(vertex).collect::<Vec<_>>(), "closed": p.closed, "const_width": p.const_width})
        }
        EntityKind::Polyline3d(p) => json!({"points": p.points.iter().map(|v| p3(*v)).collect::<Vec<_>>(), "closed": p.closed}),
        EntityKind::PolyfaceMesh(m) => json!({"vertices": m.vertices.iter().map(|v| p3(*v)).collect::<Vec<_>>(), "face_count": m.faces.len()}),
        EntityKind::Spline(s) => spline_geom(s),
        EntityKind::Ray(r) | EntityKind::XLine(r) => json!({"base": p3(r.base), "dir": unit(r.dir)}),
        EntityKind::Text(t) => Value::Object(text_geom(t)),
        EntityKind::AttDef(a) => {
            let mut g = text_geom(&a.text);
            g.insert("tag".into(), json!(a.tag));
            Value::Object(g)
        }
        EntityKind::MText(t) => json!({
            "text": t.contents, "height": t.height, "insert": p3(t.insert), "width": t.width,
            "attach": t.attach, "rotation_deg": deg(t.rotation), "style": t.style,
        }),
        EntityKind::Insert(i) => insert_geom(i),
        EntityKind::Dimension(d) => dim_geom(d),
        EntityKind::Hatch(h) => json!({"pattern": h.pattern, "solid": h.solid, "loops": h.loops.len(), "associative": h.associative}),
        EntityKind::Solid(s) | EntityKind::Trace(s) => json!({"corners": s.corners.iter().map(|c| p3(*c)).collect::<Vec<_>>()}),
        EntityKind::Face3d(f) => json!({"corners": f.corners.iter().map(|c| p3(*c)).collect::<Vec<_>>()}),
        EntityKind::Leader(l) => json!({"vertices": l.vertices.iter().map(|v| p3(*v)).collect::<Vec<_>>(), "arrow": l.arrow, "spline": l.spline}),
        _ => json!({}),
    }
}

fn record(e: &Entity, space: &str) -> Value {
    let mut r = json!({
        "space": space, "type": canonical_type(&e.kind), "handle": e.handle.hex(), "layer": e.common.layer,
        "color": color(e.common.color), "linetype": e.common.linetype, "lineweight": e.common.lineweight.to_dxf(),
        "geom": geom(&e.kind),
    });
    if let (EntityKind::Unknown(_), Some(o)) = (&e.kind, r.as_object_mut()) {
        o.insert("unsupported".into(), json!(true));
    }
    r
}

fn layer(l: &Layer) -> Value {
    json!({
        "color": color(l.color), "linetype": l.linetype, "lineweight": l.lineweight.to_dxf(),
        "on": l.on, "frozen": l.frozen, "locked": l.locked, "plot": l.plot,
    })
}

/// The canonical dump of a drawing.
pub fn dump(d: &Drawing) -> Value {
    let mut entities: Vec<Value> = d.model.iter().map(|e| record(e, "model")).collect();
    let mut paper: Vec<String> = Vec::new();
    for l in &d.layouts {
        if l.name == "Model" {
            continue;
        }
        paper.push(l.name.clone());
        entities.extend(l.entities.iter().map(|e| record(e, &format!("paper:{}", l.name))));
    }
    paper.sort();
    let mut blocks = Map::new();
    for (name, b) in &d.blocks {
        let space = format!("block:{name}");
        let mut types: std::collections::BTreeMap<String, usize> = Default::default();
        for e in b.entities.iter() {
            let r = record(e, &space);
            *types.entry(canonical_type(&e.kind)).or_default() += 1;
            entities.push(r);
        }
        blocks.insert(name.clone(), json!({"base": p3(b.base), "count": b.entities.len(), "types": types}));
    }
    let layers: Map<String, Value> = d.layers.iter().map(|l| (l.name.clone(), layer(l))).collect();
    json!({
        "tool": "cadcraft", "ok": true, "version": d.header.str("ACADVER", ""), "insunits": d.header.i64("INSUNITS", 0),
        "layers": layers, "blocks": blocks, "paper_layouts": paper, "entities": entities,
    })
}
