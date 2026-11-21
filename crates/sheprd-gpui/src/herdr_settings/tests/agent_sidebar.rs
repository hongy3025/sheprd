use super::*;
use crate::config::{SidebarLayout, sidebar::SpaceFlag};
use std::path::Path;

fn flag(flag: SpaceFlag, shown: bool) -> Edit {
    Edit::SidebarFlag { flag, shown }
}

fn layout(path: &Path) -> anyhow::Result<SidebarLayout> {
    Ok(SidebarLayout::from_daemon_config(
        &fs::read_to_string(path)?.parse()?,
    )?)
}

#[test]
fn flag_edits_preserve_rows_comments_and_other_flags() -> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("config.toml");
    let original = "# shared settings\n[ui.sidebar.spaces]\nrows = [[{ token = \"workspace\", bold = true }], [\"branch\", \"git_status\"]]\nrow_gap = 2 # keep spacing\n[ui.sidebar.agents]\nrows_by_agent.claude = [[\"agent\"]]\n";
    fs::write(&path, original)?;

    let settings = Settings::load_path(path.clone())?;
    assert_eq!(settings.sidebar_flags(), (true, true));
    let settings = settings.save(flag(SpaceFlag::PaneStrip, false))?;
    assert_eq!(settings.sidebar_flags(), (false, true));
    assert!(!layout(&path)?.spaces.pane_strip);
    assert!(layout(&path)?.spaces.status_text);

    let settings = settings.save(flag(SpaceFlag::StatusText, false))?;
    assert_eq!(settings.sidebar_flags(), (false, false));
    assert!(!layout(&path)?.spaces.pane_strip);
    assert!(!layout(&path)?.spaces.status_text);
    let settings = settings.save(flag(SpaceFlag::PaneStrip, true))?;
    assert_eq!(settings.sidebar_flags(), (true, false));
    assert!(layout(&path)?.spaces.pane_strip);
    assert!(!layout(&path)?.spaces.status_text);

    let text = fs::read_to_string(path)?;
    for retained in [
        "# shared settings",
        "{ token = \"workspace\", bold = true }",
        "row_gap = 2 # keep spacing",
        "rows_by_agent.claude = [[\"agent\"]]",
    ] {
        assert!(text.contains(retained), "{retained} in {text}");
    }
    Ok(())
}

#[test]
fn external_edit_conflicts_until_settings_are_reloaded() -> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("config.toml");
    fs::write(&path, "[ui]\nsound = false\n")?;
    let stale = Settings::load_path(path.clone())?;
    let external = "[ui]\nsound = false\nfuture = 1\n";
    fs::write(&path, external)?;

    let error = stale
        .save(flag(SpaceFlag::StatusText, false))
        .err()
        .ok_or_else(|| anyhow::anyhow!("overwrote an external change"))?;
    assert!(matches!(source(&error), Some(Error::Conflict)));
    assert_eq!(fs::read_to_string(&path)?, external);

    Settings::load_path(path.clone())?.save(flag(SpaceFlag::StatusText, false))?;
    assert!(!layout(&path)?.spaces.status_text);
    assert!(fs::read_to_string(path)?.contains("future = 1"));
    Ok(())
}
