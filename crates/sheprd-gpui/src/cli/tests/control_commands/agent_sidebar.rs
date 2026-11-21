use super::*;

#[test]
fn sidebar_commands_keep_global_options_and_bound_pane_ids() {
    let listed = parse(&["ui", "agent-sidebar", "--json", "--window", "9"]).unwrap();
    assert!(listed.json);
    assert_eq!(listed.window, Some(9));
    assert!(matches!(
        listed.command,
        ControlCommand::Ui(UiCommand::AgentSidebar)
    ));

    let recap = parse(&["ui", "agent-recap", "--pane", "w1:p1"]).unwrap();
    assert!(matches!(
        recap.command,
        ControlCommand::Ui(UiCommand::AgentRecap { pane }) if pane == "w1:p1"
    ));
    assert!(matches!(
        parse(&["ui", "agent-recap"]),
        Err(CliError::MissingValue("--pane"))
    ));
    assert!(matches!(
        parse(&["ui", "agent-recap", "--pane", ""]),
        Err(CliError::MissingValue("--pane"))
    ));
    let too_long = "x".repeat(513);
    assert!(matches!(
        parse(&["ui", "agent-recap", "--pane", &too_long]),
        Err(CliError::InvalidValue("PANE"))
    ));
}

#[cfg(any(unix, windows))]
#[test]
fn sidebar_command_rejects_non_utf8_pane_ids() {
    #[cfg(unix)]
    let invalid = {
        use std::os::unix::ffi::OsStringExt;
        std::ffi::OsString::from_vec(vec![0xff])
    };
    #[cfg(windows)]
    let invalid = {
        use std::os::windows::ffi::OsStringExt;
        OsString::from_wide(&[0xd800])
    };
    let error = LaunchOptions::parse([
        OsString::from("ui"),
        OsString::from("agent-recap"),
        OsString::from("--pane"),
        invalid.clone(),
    ])
    .unwrap_err();

    assert_eq!(error, CliError::InvalidEncoding(invalid));
}

#[test]
fn sidebar_config_defaults_to_read_and_accepts_independent_switches() {
    let read = parse(&["ui", "agent-sidebar-config"]).unwrap();
    assert!(matches!(
        read.command,
        ControlCommand::Ui(UiCommand::AgentSidebarConfig {
            pane_strip: None,
            status_text: None,
        })
    ));
    let edit = parse(&[
        "ui",
        "agent-sidebar-config",
        "--pane-strip",
        "off",
        "--status-text",
        "on",
    ])
    .unwrap();
    assert!(matches!(
        edit.command,
        ControlCommand::Ui(UiCommand::AgentSidebarConfig {
            pane_strip: Some(false),
            status_text: Some(true),
        })
    ));
}
