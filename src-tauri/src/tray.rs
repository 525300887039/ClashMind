use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager,
};

pub fn create_tray(app: &AppHandle) -> Result<(), tauri::Error> {
    let mode_rule = MenuItem::with_id(app, "mode_rule", "规则模式", true, None::<&str>)?;
    let mode_global = MenuItem::with_id(app, "mode_global", "全局模式", true, None::<&str>)?;
    let mode_direct = MenuItem::with_id(app, "mode_direct", "直连模式", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let system_proxy = MenuItem::with_id(app, "system_proxy", "系统代理", true, None::<&str>)?;
    let show = MenuItem::with_id(app, "show", "显示窗口", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &mode_rule,
            &mode_global,
            &mode_direct,
            &separator,
            &system_proxy,
            &show,
            &quit,
        ],
    )?;

    let mut builder = TrayIconBuilder::new();
    if let Some(icon) = app.default_window_icon().cloned() {
        builder = builder.icon(icon);
    }

    builder
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "quit" => app.exit(0),
            "show" => {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
            }
            id => {
                let _ = app.emit("tray-action", id);
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
            }
        })
        .build(app)?;

    Ok(())
}
