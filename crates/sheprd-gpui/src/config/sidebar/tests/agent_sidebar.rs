use super::*;

#[test]
fn summary_flags_default_on_and_override_independently() {
    let defaults = SpaceLayout::default();
    assert!(defaults.pane_strip);
    assert!(defaults.status_text);

    let pane_strip: SpaceLayout = toml::from_str("pane_strip = false").unwrap();
    assert!(!pane_strip.pane_strip);
    assert!(pane_strip.status_text);
    assert_eq!(pane_strip.rows, defaults.rows);
    assert_eq!(pane_strip.row_gap, defaults.row_gap);

    let status_text: SpaceLayout = toml::from_str("status_text = false").unwrap();
    assert!(status_text.pane_strip);
    assert!(!status_text.status_text);
    assert_eq!(status_text.rows, defaults.rows);
    assert_eq!(status_text.row_gap, defaults.row_gap);
}
