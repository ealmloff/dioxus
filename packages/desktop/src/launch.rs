use crate::{
    Config,
    app::{DesktopEventLoopState, MakeVirtualDom},
};
use dioxus_core::*;
use std::any::Any;

/// Launch the WebView and run the event loop, with configuration and root props.
///
/// This will block the main thread, and *must* be spawned on the main thread. This function does not assume any runtime
/// and is equivalent to calling launch_with_props with the tokio feature disabled.
pub fn launch_virtual_dom_blocking(
    virtual_dom: impl FnOnce() -> VirtualDom + Send + 'static,
    desktop_config: Config,
) -> ! {
    let virtual_dom = Box::new(virtual_dom);
    let (event_loop, mut event_loop_state) =
        DesktopEventLoopState::new(desktop_config, virtual_dom);

    event_loop.run(move |window_event, event_loop, control_flow| {
        *control_flow = event_loop_state.handle_event(&window_event, event_loop);
    })
}

/// Launches the WebView and runs the event loop, with configuration and root props.
pub fn launch_virtual_dom(
    virtual_dom: impl FnOnce() -> VirtualDom + Send + 'static,
    desktop_config: Config,
) -> ! {
    #[cfg(feature = "tokio_runtime")]
    {
        if let std::result::Result::Ok(handle) = tokio::runtime::Handle::try_current() {
            assert_ne!(
                handle.runtime_flavor(),
                tokio::runtime::RuntimeFlavor::CurrentThread,
                "The tokio current-thread runtime does not work with dioxus event handling"
            );
            launch_virtual_dom_blocking(virtual_dom, desktop_config);
        } else {
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(tokio::task::unconstrained(async move {
                    launch_virtual_dom_blocking(virtual_dom, desktop_config)
                }));

            unreachable!("The desktop launch function will never exit")
        }
    }

    #[cfg(not(feature = "tokio_runtime"))]
    {
        launch_virtual_dom_blocking(virtual_dom, desktop_config);
    }
}

/// Launches the WebView and runs the event loop, with configuration and root props.
pub fn launch(
    root: fn() -> Element,
    contexts: Vec<Box<dyn Fn() -> Box<dyn Any> + Send + Sync>>,
    platform_config: Vec<Box<dyn Any>>,
) -> ! {
    // Create a factory function that builds the VirtualDom with contexts
    let make_dom: MakeVirtualDom = Box::new(move || {
        let mut virtual_dom = VirtualDom::new(root);

        for context in contexts {
            virtual_dom.insert_any_root_context(context());
        }

        virtual_dom
    });

    let platform_config = *platform_config
        .into_iter()
        .find_map(|cfg| cfg.downcast::<Config>().ok())
        .unwrap_or_default();
    launch_virtual_dom(make_dom, platform_config)
}
