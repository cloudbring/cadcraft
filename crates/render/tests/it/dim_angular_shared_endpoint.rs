//! A two-line angular dimension whose lines meet at an end point they both have (found by the
//! ezdxf oracle on `uncommon.dxf`, `tools/dxf-oracle`): the legs run from the vertex to the far
//! ends of the lines, whichever of the DXF points (13/14 and 15/10) are those.

use cadcraft_doc::{DimKind, DimStyle, Dimension};
use cadcraft_geom::Vec3;
use cadcraft_render::dimension_geometry;

/// The dimension as the file has it: line 1 runs 13 -> 14, line 2 runs 15 -> 10, and both lines
/// meet at 14 = 15. The angle between them as ezdxf reads it is 2.2019 radians (126.2 degrees,
/// the side the arc point 16 is on).
fn shared_endpoint() -> Dimension {
    Dimension {
        kind: DimKind::Angular,
        defpt: Vec3::new(52.94913262992776, 50.94479491088032, 0.0),
        text_mid: Vec3::new(53.62410678800024, 57.23356849952836, 0.0),
        p13: Vec3::new(61.08398925804994, 50.94479491088032, 0.0),
        p14: Vec3::new(52.94913262992776, 56.89087154104865, 0.0),
        p15: Vec3::new(52.94913262992776, 56.89087154104865, 0.0),
        p16: Vec3::new(53.70212415654574, 56.9685573200461, 0.0),
        text: String::new(),
        style: "Standard".into(),
        measurement: 2.201985786565511,
        text_rotation: 0.0,
        user_text_pos: false,
        block: None,
        overrides: Default::default(),
        assoc: Vec::new(),
        marks: Default::default(),
        insertion_point: Vec3::ZERO,
    }
}

#[test]
fn legs_are_the_far_ends_when_the_lines_share_an_end_point() {
    let g = dimension_geometry(&shared_endpoint(), &DimStyle::default(), 1.0);
    assert!((g.measurement - 2.201985786565511).abs() < 1e-9, "measured {} radians", g.measurement);
}
