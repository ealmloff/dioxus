//! Testing support for driving the normal desktop renderer from integration tests.

use crate::{
    app::{DesktopEventLoopState, MakeVirtualDom},
    config::Config,
};
use dioxus_core::VirtualDom;
use std::time::{Duration, Instant};
use tao::{
    event_loop::{ControlFlow, EventLoop},
    platform::run_return::EventLoopExtRunReturn,
};

/// A manually-pumped desktop renderer harness for tests.
pub struct DesktopTestHarness {
    event_loop: EventLoop<crate::ipc::UserWindowEvent>,
    event_loop_state: DesktopEventLoopState,
}

impl DesktopTestHarness {
    /// Create a new harness around the normal desktop renderer.
    pub fn new(
        virtual_dom: impl FnOnce() -> VirtualDom + Send + 'static,
        desktop_config: Config,
    ) -> Self {
        let virtual_dom: MakeVirtualDom = Box::new(virtual_dom);
        let (event_loop, event_loop_state) =
            DesktopEventLoopState::new(desktop_config, virtual_dom);
        Self {
            event_loop,
            event_loop_state,
        }
    }

    /// Pump the desktop event loop until `poll` returns a value or the timeout expires.
    pub fn run_until<T>(
        &mut self,
        timeout: Duration,
        mut poll: impl FnMut() -> Option<T>,
    ) -> Option<T> {
        let deadline = Instant::now() + timeout;

        loop {
            if let Some(value) = poll() {
                return Some(value);
            }

            if Instant::now() >= deadline {
                return None;
            }

            let event_loop_state = &mut self.event_loop_state;
            let mut response = None;
            self.event_loop
                .run_return(|event, event_loop, control_flow| {
                    let app_control_flow = event_loop_state.handle_event(&event, event_loop);

                    if let Some(value) = poll() {
                        response = Some(value);
                        *control_flow = ControlFlow::Exit;
                    } else if Instant::now() >= deadline {
                        *control_flow = ControlFlow::Exit;
                    } else if matches!(app_control_flow, ControlFlow::Exit) {
                        *control_flow = ControlFlow::Exit;
                    } else {
                        *control_flow =
                            ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(10));
                    }
                });

            if let Some(value) = response {
                return Some(value);
            }
        }
    }

    /// Ask the renderer to shut down and briefly pump the event loop.
    pub fn shutdown(&mut self) {
        self.event_loop_state.shutdown();
        let _: Option<()> = self.run_until(Duration::from_millis(100), || None);
    }
}

impl Drop for DesktopTestHarness {
    fn drop(&mut self) {
        self.shutdown();
    }
}
