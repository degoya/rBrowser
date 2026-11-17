//! The renderer side: `window.cefQuery` exists only in the shell's own pages
//! (`rbrowser://ui/`); the pages under test never see it.

use std::os::raw::c_int;
use std::sync::{Arc, OnceLock};

use cef::wrapper::message_router::{
    MessageRouterConfig, MessageRouterRendererSide, MessageRouterRendererSideHandlerCallbacks,
    RendererSideRouter,
};
use cef::*;

use crate::shell::is_shell_url;

static ROUTER: OnceLock<Arc<RendererSideRouter>> = OnceLock::new();

fn router() -> &'static Arc<RendererSideRouter> {
    ROUTER.get_or_init(|| RendererSideRouter::new(MessageRouterConfig::default()))
}

fn is_shell_frame(frame: &Frame) -> bool {
    frame.is_main() != 0 && is_shell_url(&CefString::from(&frame.url()).to_string())
}

wrap_render_process_handler! {
    pub struct ShellRenderer {}

    impl RenderProcessHandler {
        fn on_context_created(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            context: Option<&mut V8Context>,
        ) {
            let Some(frame) = frame else {
                return;
            };
            if is_shell_frame(frame) {
                router().on_context_created(
                    browser.map(|browser| browser.clone()),
                    Some(frame.clone()),
                    context.map(|context| context.clone()),
                );
            }
        }

        fn on_context_released(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            context: Option<&mut V8Context>,
        ) {
            let Some(frame) = frame else {
                return;
            };
            if is_shell_frame(frame) {
                router().on_context_released(
                    browser.map(|browser| browser.clone()),
                    Some(frame.clone()),
                    context.map(|context| context.clone()),
                );
            }
        }

        fn on_process_message_received(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            source_process: ProcessId,
            message: Option<&mut ProcessMessage>,
        ) -> c_int {
            c_int::from(router().on_process_message_received(
                browser.map(|browser| browser.clone()),
                frame.map(|frame| frame.clone()),
                Some(source_process),
                message.map(|message| message.clone()),
            ))
        }
    }
}
