#[cfg(target_os = "macos")]
use objc2::{
    runtime::{AnyObject, Bool, NSObjectProtocol},
    ClassType,
};
#[cfg(target_os = "macos")]
use objc2_app_kit::{
    NSApplication, NSApplicationActivationOptions, NSColor, NSPanel, NSRunningApplication,
    NSScreenSaverWindowLevel, NSWindow, NSWindowCollectionBehavior, NSWindowStyleMask,
};
#[cfg(target_os = "macos")]
use objc2_foundation::{NSNumber, NSString};
#[cfg(target_os = "macos")]
use objc2_web_kit::WKWebView;
#[cfg(target_os = "macos")]
use tauri::Manager;

#[cfg(target_os = "macos")]
objc2::define_class!(
    #[unsafe(super(NSPanel))]
    #[name = "PromptDrawerOverlayPanel"]
    struct PromptDrawerOverlayPanel;

    unsafe impl NSObjectProtocol for PromptDrawerOverlayPanel {}

    impl PromptDrawerOverlayPanel {
        #[unsafe(method(canBecomeKeyWindow))]
        fn can_become_key_window(&self) -> bool {
            false
        }

        #[unsafe(method(canBecomeMainWindow))]
        fn can_become_main_window(&self) -> bool {
            false
        }
    }
);

#[cfg(target_os = "macos")]
pub(crate) fn run_on_main_thread_sync<T, F>(app: &tauri::AppHandle, task: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    if objc2::MainThreadMarker::new().is_some() {
        return task();
    }

    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    app.run_on_main_thread(move || {
        let _ = sender.send(task());
    })
    .map_err(|error| format!("Failed to schedule macOS window task: {error}"))?;

    receiver
        .recv()
        .map_err(|_| "macOS window task ended without a result".to_string())?
}

#[cfg(target_os = "macos")]
pub(crate) fn activate_running_application(
    app: &tauri::AppHandle,
    pid: u32,
) -> Result<(), String> {
    run_on_main_thread_sync(app, move || {
        let running = NSRunningApplication::runningApplicationWithProcessIdentifier(pid as i32)
            .ok_or_else(|| format!("Target process {pid} is no longer running."))?;
        running
            .activateWithOptions(NSApplicationActivationOptions::empty())
            .then_some(())
            .ok_or_else(|| format!("Target process {pid} could not be activated."))
    })
}

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
    let app = window.app_handle().clone();
    let window = window.clone();
    run_on_main_thread_sync(&app, move || {
        configure_non_activating_panel_on_main_thread(&window)
    })
}

#[cfg(target_os = "macos")]
fn configure_non_activating_panel_on_main_thread(
    window: &tauri::WebviewWindow,
) -> Result<(), String> {
    let _mtm = objc2::MainThreadMarker::new()
        .ok_or_else(|| "configure_non_activating_panel must run on the main thread".to_string())?;
    let ns_window_ptr = window.ns_window().map_err(|e| e.to_string())?;
    if ns_window_ptr.is_null() {
        return Err("ns_window returned null".to_string());
    }

    unsafe {
        let ns_window = &*(ns_window_ptr.cast::<NSWindow>());
        let object: &AnyObject = ns_window.as_ref();
        let original_class_name = object.class().name().to_string_lossy().to_string();
        let action = ensure_native_overlay_panel(window, &original_class_name)?;
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
        let panel: &NSPanel = &*(ns_window_ptr.cast::<NSPanel>());
        panel.setFloatingPanel(true);
        panel.setBecomesKeyOnlyIfNeeded(true);
        ns_window.orderFrontRegardless();

        let is_native_panel: bool = objc2::msg_send![ns_window, isKindOfClass: NSPanel::class()];
        let can_become_key: Bool = objc2::msg_send![ns_window, canBecomeKeyWindow];
        let can_become_main: Bool = objc2::msg_send![ns_window, canBecomeMainWindow];
        if !is_native_panel || can_become_key.as_bool() || can_become_main.as_bool() {
            return Err(format!(
                "Overlay {} is not a non-activating native NSPanel.",
                window.label()
            ));
        }

        if focus_diagnostics_enabled() {
            let report = PanelKeyBehaviorReport {
                label: window.label().to_string(),
                class_name: object.class().name().to_string_lossy().to_string(),
                action,
                can_become_key: Some(can_become_key.as_bool()),
                can_become_main: Some(can_become_main.as_bool()),
            };
            eprintln!("{}", format_panel_key_behavior_report(&report));
        }
    }

    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PanelClassAction {
    AlreadyNativePanel,
    ConvertToNativePanel,
}

fn panel_class_action_for_name(class_name: &str) -> PanelClassAction {
    if class_name.contains("PromptDrawerOverlayPanel") {
        PanelClassAction::AlreadyNativePanel
    } else {
        PanelClassAction::ConvertToNativePanel
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PanelKeyBehaviorReport {
    label: String,
    class_name: String,
    action: PanelClassAction,
    can_become_key: Option<bool>,
    can_become_main: Option<bool>,
}

fn format_panel_key_behavior_report(report: &PanelKeyBehaviorReport) -> String {
    format!(
        "prompt-picker-panel label={} class={} action={:?} can_become_key={} can_become_main={}",
        report.label,
        report.class_name,
        report.action,
        option_bool_label(report.can_become_key),
        option_bool_label(report.can_become_main)
    )
}

fn option_bool_label(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "true",
        Some(false) => "false",
        None => "unknown",
    }
}

fn focus_diagnostics_enabled() -> bool {
    std::env::var("PROMPT_PICKER_FOCUS_DIAGNOSTICS").is_ok()
}

#[cfg(target_os = "macos")]
fn ensure_native_overlay_panel(
    window: &tauri::WebviewWindow,
    class_name: &str,
) -> Result<PanelClassAction, String> {
    let action = panel_class_action_for_name(class_name);
    if action == PanelClassAction::ConvertToNativePanel {
        let ns_window_ptr = window.ns_window().map_err(|error| error.to_string())?;
        if ns_window_ptr.is_null() {
            return Err("ns_window returned null".to_string());
        }
        unsafe {
            let object = &*(ns_window_ptr.cast::<AnyObject>());
            let current_class = object.class();
            let panel_class = PromptDrawerOverlayPanel::class();
            if panel_class.instance_size() > current_class.instance_size() {
                return Err(
                    "Native NSPanel class does not fit the Tauri window allocation.".to_string(),
                );
            }

            // The window is still hidden here. Replace only its Objective-C class;
            // the NSWindow object, WKWebView, delegate, and ownership stay unchanged.
            let previous_class = AnyObject::set_class(object, panel_class);
            if previous_class.name().to_string_lossy() != class_name {
                return Err("Overlay window class changed during NSPanel conversion.".to_string());
            }
        }
    }
    Ok(action)
}

#[cfg(target_os = "macos")]
pub fn configure_transparent_webview_window(window: &tauri::WebviewWindow) -> Result<(), String> {
    let app = window.app_handle().clone();
    let window = window.clone();
    run_on_main_thread_sync(&app, move || {
        configure_transparent_webview_window_on_main_thread(&window)
    })
}

#[cfg(target_os = "macos")]
fn configure_transparent_webview_window_on_main_thread(
    window: &tauri::WebviewWindow,
) -> Result<(), String> {
    let _mtm = objc2::MainThreadMarker::new().ok_or_else(|| {
        "configure_transparent_webview_window must run on the main thread".to_string()
    })?;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_class_action_keeps_existing_native_panel() {
        assert_eq!(
            panel_class_action_for_name("PromptDrawerOverlayPanel"),
            PanelClassAction::AlreadyNativePanel
        );
    }

    #[test]
    fn panel_class_action_converts_tao_wry_to_native_panel() {
        assert_eq!(
            panel_class_action_for_name("TaoWindow"),
            PanelClassAction::ConvertToNativePanel
        );
        assert_eq!(
            panel_class_action_for_name("WryWindow"),
            PanelClassAction::ConvertToNativePanel
        );
    }

    #[test]
    fn panel_diagnostic_format_includes_key_behavior() {
        let report = PanelKeyBehaviorReport {
            label: "prompt-popover".to_string(),
            class_name: "TaoWindow".to_string(),
            action: PanelClassAction::ConvertToNativePanel,
            can_become_key: Some(true),
            can_become_main: Some(true),
        };

        let formatted = format_panel_key_behavior_report(&report);

        assert!(formatted.contains("prompt-popover"));
        assert!(formatted.contains("TaoWindow"));
        assert!(formatted.contains("can_become_key=true"));
        assert!(formatted.contains("can_become_main=true"));
    }

    #[test]
    fn non_activating_panel_configuration_uses_native_nspanel() {
        let source = include_str!("macos_panels.rs");

        assert!(source.contains("PromptDrawerOverlayPanel"));
        assert!(source.contains("NSPanel::class()"));
        assert!(source.contains("isKindOfClass"));
    }

    #[test]
    fn main_window_activation_remains_separate_from_overlay_configuration() {
        let source = include_str!("macos_panels.rs");

        assert!(source.contains("pub fn activate_main_window"));
        assert!(source.contains("pub fn configure_non_activating_panel"));
    }

    #[test]
    fn native_window_configuration_is_dispatched_to_the_main_thread() {
        let source = include_str!("macos_panels.rs");

        assert!(source.contains("pub(crate) fn run_on_main_thread_sync"));
        assert!(source.contains("configure_non_activating_panel_on_main_thread"));
        assert!(source.contains("configure_transparent_webview_window_on_main_thread"));
        assert!(source.matches("run_on_main_thread_sync").count() >= 3);
        assert!(source.matches("MainThreadMarker::new()").count() >= 3);
    }
}
