use super::*;

#[test]
fn no_agent_recap_has_the_documented_no_result_exit_code() {
    let error: crate::Error = crate::control::Error::Rejected {
        code: ErrorCode::NoResult,
        message: "No idle recap is available".into(),
    }
    .into();
    assert_eq!(status(&error), exit::NOTHING);
    assert_eq!(code(&error), "no_result");
}
