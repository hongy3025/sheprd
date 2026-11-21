use super::*;
use crate::{config::sidebar::SpaceFlag, herdr_settings::Edit};

fn recorded_flags(
    view: &Entity<SettingsWindow>,
    cx: &mut VisualTestContext,
) -> Vec<(SpaceFlag, bool)> {
    view.read_with(cx, |view, _| {
        view.plugins
            .edits
            .iter()
            .flatten()
            .filter_map(|edit| match edit {
                Edit::SidebarFlag { flag, shown } => Some((*flag, *shown)),
                _ => None,
            })
            .collect()
    })
}

#[gpui::test]
fn workspace_summary_switches_edit_flags_independently(cx: &mut TestAppContext) {
    let (view, cx) = open(cx);
    view.update(cx, |view, cx| {
        view.shared = Some(herdr_settings::Settings::parse_text("").unwrap());
        cx.notify();
    });
    draw(cx);
    for id in [
        "plugin-sidebar-flag-pane-strip",
        "plugin-sidebar-flag-status-text",
    ] {
        assert!(cx.debug_bounds(id).is_some(), "{id}");
    }

    click(cx, "plugin-sidebar-flag-pane-strip");
    click(cx, "plugin-sidebar-flag-status-text");

    let expected = if cfg!(unix) {
        vec![
            (SpaceFlag::PaneStrip, false),
            (SpaceFlag::StatusText, false),
        ]
    } else {
        Vec::new()
    };
    assert_eq!(recorded_flags(&view, cx), expected);

    view.update(cx, |view, cx| {
        view.saving = true;
        cx.notify();
    });
    click(cx, "plugin-sidebar-flag-pane-strip");
    assert_eq!(recorded_flags(&view, cx), expected);
}
