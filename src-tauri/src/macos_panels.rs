#[cfg(target_os = "macos")]
use objc2_app_kit::{
    NSApplication, NSColor, NSScreenSaverWindowLevel, NSWindow, NSWindowCollectionBehavior,
    NSWindowStyleMask,
};
#[cfg(target_os = "macos")]
use objc2_foundation::{NSNumber, NSString};
#[cfg(target_os = "macos")]
use objc2_web_kit::WKWebView;

#[cfg(target_os = "macos")]
pub fn activate_main_window(window: &tauri::WebviewWindow) -> Result<(), String> {
    let mtm = objc2::MainThreadMarker::new()
        .ok_or_else(|| "activate_main_window must run on the main thread".to_string())?;
    let ns_window_ptr = window.ns_window().map_err(|e| e.to_string())?;
    if ns_window_ptr.is_null() {
        return Err("ns_window returned null".to_string());
    }

    unsafe {
        let app = NSApplication::sharedApplication(mtm);
        #[allow(deprecated)]
        app.activateIgnoringOtherApps(true);

        let ns_window = &*(ns_window_ptr.cast::<NSWindow>());
        ns_window.makeKeyAndOrderFront(None);
        ns_window.makeMainWindow();
        ns_window.makeKeyWindow();
    }

    Ok(())
}

#[cfg(target_os = "macos")]
pub fn configure_non_activating_panel(window: &tauri::WebviewWindow) -> Result<(), String> {
    let ns_window_ptr = window.ns_window().map_err(|e| e.to_string())?;
    if ns_window_ptr.is_null() {
        return Err("ns_window returned null".to_string());
    }

    unsafe {
        let ns_window = &*(ns_window_ptr.cast::<NSWindow>());
        let mask = ns_window.styleMask()
            | NSWindowStyleMask::NonactivatingPanel
            | NSWindowStyleMask::UtilityWindow;
        ns_window.setStyleMask(mask);
        ns_window.setLevel(NSScreenSaverWindowLevel);
        ns_window.setCanHide(false);
        ns_window.setHidesOnDeactivate(false);
        ns_window.setIgnoresMouseEvents(false);
        ns_window.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::CanJoinAllApplications
                | NSWindowCollectionBehavior::FullScreenAuxiliary
                | NSWindowCollectionBehavior::Stationary
                | NSWindowCollectionBehavior::Transient
                | NSWindowCollectionBehavior::IgnoresCycle,
        );
        ns_window.orderFrontRegardless();
    }

    Ok(())
}

#[cfg(target_os = "macos")]
pub fn configure_transparent_webview_window(window: &tauri::WebviewWindow) -> Result<(), String> {
    let ns_window_ptr = window.ns_window().map_err(|e| e.to_string())?;
    if ns_window_ptr.is_null() {
        return Err("ns_window returned null".to_string());
    }

    unsafe {
        let ns_window = &*(ns_window_ptr.cast::<NSWindow>());
        let clear = NSColor::clearColor();
        ns_window.setOpaque(false);
        ns_window.setBackgroundColor(Some(&clear));
        ns_window.setHasShadow(false);
    }

    window
        .with_webview(|webview| unsafe {
            let view: &WKWebView = &*webview.inner().cast();
            let draws_background = NSNumber::new_bool(false);
            let key = NSString::from_str("drawsBackground");
            let _: () = objc2::msg_send![view, setValue: &*draws_background, forKey: &*key];
        })
        .map_err(|e| e.to_string())
}

#[cfg(not(target_os = "macos"))]
pub fn activate_main_window(_window: &tauri::WebviewWindow) -> Result<(), String> {
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn configure_non_activating_panel(_window: &tauri::WebviewWindow) -> Result<(), String> {
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn configure_transparent_webview_window(_window: &tauri::WebviewWindow) -> Result<(), String> {
    Ok(())
}
