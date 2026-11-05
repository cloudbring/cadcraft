//! Release 12 binary DXF (found by the ezdxf oracle, `tools/dxf-oracle`): group codes are one
//! byte (255 introduces a two-byte code), not the two bytes of R13 and later.

use cadcraft_dxf::{Tag, is_binary, parse};

fn r12_file() -> Vec<u8> {
    let mut b = b"AutoCAD Binary DXF\r\n\x1a\0".to_vec();
    let mut s = |code: u8, text: &str| {
        b.push(code);
        b.extend_from_slice(text.as_bytes());
        b.push(0);
    };
    s(0, "SECTION");
    s(2, "ENTITIES");
    s(0, "LINE");
    s(8, "WALLS");
    b.push(10);
    b.extend_from_slice(&1.5f64.to_le_bytes());
    b.push(62);
    b.extend_from_slice(&3i16.to_le_bytes());
    // Group codes of 255 and above: 255 then the two-byte code.
    b.push(255);
    b.extend_from_slice(&1000i16.to_le_bytes());
    b.extend_from_slice(b"note\0");
    let mut s = |code: u8, text: &str| {
        b.push(code);
        b.extend_from_slice(text.as_bytes());
        b.push(0);
    };
    s(0, "ENDSEC");
    s(0, "EOF");
    b
}

#[test]
fn r12_binary_has_one_byte_group_codes() {
    let bytes = r12_file();
    assert!(is_binary(&bytes));
    let want = vec![
        Tag::s(0, "SECTION"),
        Tag::s(2, "ENTITIES"),
        Tag::s(0, "LINE"),
        Tag::s(8, "WALLS"),
        Tag::f(10, 1.5),
        Tag::i(62, 3),
        Tag::s(1000, "note"),
        Tag::s(0, "ENDSEC"),
        Tag::s(0, "EOF"),
    ];
    assert_eq!(parse(&bytes).unwrap(), want);
}
