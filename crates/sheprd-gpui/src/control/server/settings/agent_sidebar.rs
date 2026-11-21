use super::{answer_when_done, socket};
use crate::{
    control::protocol::{ErrorCode, Response, settings::Reply},
    herdr_settings::{Edit, Settings},
};
use std::time::Duration;

pub(super) fn serve(
    pane_strip: Option<bool>,
    status_text: Option<bool>,
    wait: Duration,
    incoming: socket::Incoming,
    watches: &mut super::Watches,
    cx: &mut gpui::App,
) {
    if (pane_strip.is_some() || status_text.is_some()) && !cfg!(unix) {
        incoming.respond(Response::error(
            ErrorCode::Unsupported,
            "Saving Herdr's shared config is unavailable on this platform",
        ));
        return;
    }
    let task = cx.background_executor().spawn(async move {
        let settings = Settings::load().and_then(|settings| {
            let settings = match pane_strip {
                Some(shown) => settings.save(Edit::SidebarFlag {
                    flag: crate::config::SpaceFlag::PaneStrip,
                    shown,
                })?,
                None => settings,
            };
            match status_text {
                Some(shown) => settings.save(Edit::SidebarFlag {
                    flag: crate::config::SpaceFlag::StatusText,
                    shown,
                }),
                None => Ok(settings),
            }
        });
        match settings {
            Ok(settings) => {
                let (pane_strip, status_text) = settings.sidebar_flags();
                Response::Settings {
                    reply: Reply::AgentSidebar {
                        pane_strip,
                        status_text,
                    },
                }
            }
            Err(error) => Response::error(ErrorCode::Failed, error.to_string()),
        }
    });
    answer_when_done(task, wait, incoming, watches, cx);
}
