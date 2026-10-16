#![cfg_attr(windows, windows_subsystem = "windows")]

use anyhow::{Context, Result};
use qbs::{
    browser::{BrowserId, BrowserSnapshot},
    config::{self, Config, SwitchMode},
    platform,
    switching::{self, SwitchResult},
    ui::{
        menu::{ContextMenu, MenuAction},
        renderer,
    },
};
use softbuffer::{Context as SoftContext, Surface};
use std::{num::NonZeroU32, rc::Rc};
use tao::{
    dpi::{PhysicalPosition, PhysicalSize},
    event::{ElementState, Event, MouseButton, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    window::WindowBuilder,
};
use tray_icon::{TrayIcon, TrayIconBuilder};

#[derive(Debug)]
enum UserEvent {
    Menu(muda::MenuId),
    Tray,
}

#[cfg(windows)]
fn another_instance_running() -> bool {
    use windows::core::w;
    use windows::Win32::{
        Foundation::{GetLastError, ERROR_ALREADY_EXISTS},
        System::Threading::CreateMutexW,
    };
    unsafe {
        let _ = CreateMutexW(None, true, w!("Local\\QuickBrowserSwitcherSingleInstance"));
        GetLastError() == ERROR_ALREADY_EXISTS
    }
}

#[cfg(not(windows))]
fn another_instance_running() -> bool {
    false
}

fn parse_browser_arg() -> (Option<BrowserId>, bool) {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let force_supported = args.iter().any(|a| a == "--supported");
    let browser = args
        .windows(2)
        .find(|w| w[0] == "--browser")
        .and_then(|w| BrowserId::parse(&w[1]));
    (browser, force_supported)
}

fn run_switch(
    snapshot: &BrowserSnapshot,
    config: &mut Config,
    target: BrowserId,
    force_supported: bool,
) -> SwitchResult {
    let mode = if force_supported {
        SwitchMode::Supported
    } else {
        config.switch_mode
    };
    config.last_selected_browser = Some(target.as_str().into());
    let result = switching::switch(
        snapshot,
        target,
        mode,
        config.experimental_warning_acknowledged,
    )
    .unwrap_or_else(|e| SwitchResult {
        kind: switching::SwitchResultKind::VerificationFailed,
        message: format!("Switch failed: {e:#}"),
    });
    let _ = config::save(config);
    result
}

fn rebuild_menu(
    config: &Config,
    snapshot: &BrowserSnapshot,
    tray: &TrayIcon,
) -> Result<ContextMenu> {
    let menu = ContextMenu::new(config, snapshot)?;
    tray.set_menu(Some(Box::new(menu.menu.clone())));
    Ok(menu)
}

fn main() -> Result<()> {
    if another_instance_running() {
        return Ok(());
    }
    if std::env::var_os("QBS_LOG").is_some() {
        tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .init();
    }
    let mut config = config::load();
    let mut snapshot = BrowserSnapshot::discover();
    let (requested, force_supported) = parse_browser_arg();
    let mut feedback =
        requested.map(|target| run_switch(&snapshot, &mut config, target, force_supported));

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let position = config
        .window_x
        .zip(config.window_y)
        .map(|(x, y)| PhysicalPosition::new(x, y));
    let mut builder = WindowBuilder::new()
        .with_title("Quick Browser Switcher")
        .with_inner_size(PhysicalSize::new(renderer::WIDTH, renderer::HEIGHT))
        .with_resizable(false)
        .with_decorations(false)
        .with_transparent(false)
        .with_always_on_top(config.always_on_top)
        .with_visible(true);
    #[cfg(windows)]
    {
        use tao::platform::windows::WindowBuilderExtWindows;
        builder = builder.with_skip_taskbar(true);
    }
    if let Some(position) = position {
        builder = builder.with_position(position);
    }
    let window = Rc::new(builder.build(&event_loop).context("create QBS window")?);
    #[cfg(windows)]
    {
        use tao::platform::windows::WindowExtWindows;
        platform::style_widget_window(window.hwnd(), config.opacity)?;
    }
    let soft_context = SoftContext::new(window.clone())
        .map_err(|e| anyhow::anyhow!("create CPU renderer: {e:?}"))?;
    let mut surface = Surface::new(&soft_context, window.clone())
        .map_err(|e| anyhow::anyhow!("create render surface: {e:?}"))?;

    let placeholder = muda::Menu::new();
    let tray = TrayIconBuilder::new()
        .with_tooltip(format!("QBS — {}", snapshot.current.label()))
        .with_icon(renderer::tray_icon(&snapshot.current)?)
        .with_menu(Box::new(placeholder))
        .build()
        .context("create tray icon")?;
    let mut menu = rebuild_menu(&config, &snapshot, &tray)?;
    {
        let proxy = proxy.clone();
        muda::MenuEvent::set_event_handler(Some(move |event: muda::MenuEvent| {
            let _ = proxy.send_event(UserEvent::Menu(event.id));
        }));
    }
    {
        let proxy = proxy.clone();
        tray_icon::TrayIconEvent::set_event_handler(Some(move |event| {
            if let tray_icon::TrayIconEvent::Click {
                button: tray_icon::MouseButton::Left,
                button_state: tray_icon::MouseButtonState::Up,
                ..
            } = event
            {
                let _ = proxy.send_event(UserEvent::Tray);
            }
        }));
    }

    let mut cursor = (0.0, 0.0);
    let mut dragging = false;
    let mut visible = true;
    window.request_redraw();
    event_loop.run(move |event, _target, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::UserEvent(UserEvent::Tray) => {
                visible = !visible;
                window.set_visible(visible);
                if visible {
                    window.request_redraw();
                }
            }
            Event::UserEvent(UserEvent::Menu(id)) => {
                if let Some(action) = menu.action_for(&id) {
                    match action {
                        MenuAction::Switch(target) => {
                            feedback = Some(run_switch(&snapshot, &mut config, target, false));
                            snapshot = BrowserSnapshot::discover();
                        }
                        MenuAction::SetMode(mode) => {
                            if mode == SwitchMode::Experimental
                                && !config.experimental_warning_acknowledged
                            {
                                platform::show_message(
                                    "Experimental switching",
                                    "Silent switching modifies four protected per-user associations (HTTP, HTTPS, .htm and .html). Windows can reject this at any time. QBS verifies every association and opens Default Apps if verification fails. Acknowledge the warning in the menu before using it.",
                                );
                            }
                            config.switch_mode = mode;
                            let _ = config::save(&config);
                        }
                        MenuAction::AcknowledgeExperimental => {
                            config.experimental_warning_acknowledged =
                                !config.experimental_warning_acknowledged;
                            let _ = config::save(&config);
                        }
                        MenuAction::OpenSettings => {
                            let _ = platform::open_uri("ms-settings:defaultapps");
                        }
                        MenuAction::Refresh => {
                            snapshot = BrowserSnapshot::discover();
                            feedback = None;
                        }
                        MenuAction::ToggleLock => {
                            config.window_locked = !config.window_locked;
                            let _ = config::save(&config);
                        }
                        MenuAction::ToggleAlwaysOnTop => {
                            config.always_on_top = !config.always_on_top;
                            window.set_always_on_top(config.always_on_top);
                            let _ = config::save(&config);
                        }
                        MenuAction::ToggleStartup => {
                            let wanted = !config.start_with_windows;
                            if platform::set_start_with_windows(wanted).is_ok() {
                                config.start_with_windows = wanted;
                                let _ = config::save(&config);
                            }
                        }
                        MenuAction::SetTheme(theme) => {
                            config.theme = theme;
                            let _ = config::save(&config);
                        }
                        MenuAction::SetOpacity(value) => {
                            config.opacity = value as f32 / 100.0;
                            let _ = config::save(&config);
                            #[cfg(windows)]
                            {
                                use tao::platform::windows::WindowExtWindows;
                                let _ = platform::style_widget_window(window.hwnd(), config.opacity);
                            }
                        }
                        MenuAction::ShowHide => {
                            visible = !visible;
                            window.set_visible(visible);
                        }
                        MenuAction::About => {
                            platform::show_message(
                                "Quick Browser Switcher",
                                "Quick Browser Switcher (QBS) 0.1.0\n\nA lightweight Windows 11 widget for Chrome and Edge.\nMIT licensed. No telemetry, service, WebView, or elevation required.",
                            );
                        }
                        MenuAction::Quit => {
                            *control_flow = ControlFlow::Exit;
                            return;
                        }
                    }
                    if let Ok(updated) = rebuild_menu(&config, &snapshot, &tray) {
                        menu = updated;
                    }
                    let _ = tray.set_tooltip(Some(format!("QBS — {}", snapshot.current.label())));
                    if let Ok(icon) = renderer::tray_icon(&snapshot.current) {
                        let _ = tray.set_icon(Some(icon));
                    }
                    window.request_redraw();
                }
            }
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    visible = false;
                    window.set_visible(false);
                }
                WindowEvent::Focused(true) => {
                    snapshot = BrowserSnapshot::discover();
                    window.request_redraw();
                }
                WindowEvent::CursorMoved { position, .. } => {
                    cursor = (position.x, position.y);
                }
                WindowEvent::MouseInput {
                    state: ElementState::Pressed,
                    button: MouseButton::Left,
                    ..
                } => {
                    if renderer::hit_button(cursor.0, cursor.1) {
                        if let Some(target) = snapshot.target() {
                            feedback = Some(run_switch(&snapshot, &mut config, target, false));
                            snapshot = BrowserSnapshot::discover();
                            window.request_redraw();
                        }
                    } else if !config.window_locked {
                        dragging = window.drag_window().is_ok();
                    }
                }
                WindowEvent::MouseInput {
                    state: ElementState::Released,
                    button: MouseButton::Left,
                    ..
                } => {
                    if std::mem::take(&mut dragging) {
                        if let Ok(position) = window.outer_position() {
                            #[cfg(windows)]
                            let (x, y) = {
                                use tao::platform::windows::WindowExtWindows;
                                platform::snap_window(
                                    window.hwnd(),
                                    (position.x, position.y),
                                    (renderer::WIDTH as i32, renderer::HEIGHT as i32),
                                    18,
                                )
                            };
                            #[cfg(not(windows))]
                            let (x, y) = (position.x, position.y);
                            window.set_outer_position(PhysicalPosition::new(x, y));
                            config.window_x = Some(x);
                            config.window_y = Some(y);
                            let _ = config::save(&config);
                        }
                    }
                }
                WindowEvent::MouseInput {
                    state: ElementState::Pressed,
                    button: MouseButton::Right,
                    ..
                } => {
                    #[cfg(windows)]
                    {
                        use muda::ContextMenu as _;
                        use tao::platform::windows::WindowExtWindows;
                        unsafe {
                            menu.menu.show_context_menu_for_hwnd(window.hwnd(), None);
                        }
                    }
                }
                _ => {}
            },
            Event::RedrawRequested(_) => {
                if let Ok(pm) = renderer::render(&snapshot, &config, feedback.as_ref()) {
                    let width = NonZeroU32::new(renderer::WIDTH).unwrap();
                    let height = NonZeroU32::new(renderer::HEIGHT).unwrap();
                    if surface.resize(width, height).is_ok() {
                        if let Ok(mut buffer) = surface.buffer_mut() {
                            renderer::pixmap_to_softbuffer(&pm, &mut buffer);
                            let _ = buffer.present();
                        }
                    }
                }
            }
            _ => {}
        }
    });
}
