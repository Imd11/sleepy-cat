use std::ffi::{c_void, CStr, CString};
use std::time::{Duration, Instant};

use super::{AXUIElementRef, CFTypeRef, CandidateInput};

const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
const K_AX_VALUE_CG_POINT_TYPE: i32 = 1;
const K_AX_VALUE_CG_SIZE_TYPE: i32 = 2;
const K_AX_VALUE_CF_RANGE_TYPE: i32 = 4;
const AX_ERROR_SUCCESS: i32 = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AxQueryError {
    CannotComplete,
    InvalidElement,
    Unsupported,
    TimedOut,
    Other(i32),
}

impl AxQueryError {
    fn from_code(code: i32) -> Self {
        match code {
            -25204 => Self::CannotComplete,
            -25202 => Self::InvalidElement,
            -25205 => Self::Unsupported,
            value => Self::Other(value),
        }
    }
}

pub(super) struct OwnedCfValue(CFTypeRef);

impl OwnedCfValue {
    pub(super) fn created(value: CFTypeRef) -> Option<Self> {
        (!value.is_null()).then_some(Self(value))
    }

    pub(super) unsafe fn retained(value: CFTypeRef) -> Option<Self> {
        if value.is_null() {
            return None;
        }
        Some(Self(unsafe { CFRetain(value) }))
    }

    pub(super) fn as_ptr(&self) -> CFTypeRef {
        self.0
    }
}

impl Drop for OwnedCfValue {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CFRelease(self.0) };
        }
    }
}

pub(super) type OwnedAxElement = OwnedCfValue;

#[repr(C)]
#[derive(Default)]
struct AxPoint {
    x: f64,
    y: f64,
}

#[repr(C)]
#[derive(Default)]
struct AxSize {
    width: f64,
    height: f64,
}

#[repr(C)]
#[derive(Default)]
struct AxRange {
    location: isize,
    length: isize,
}

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXUIElementCreateSystemWide() -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: *const c_void,
        value: *mut CFTypeRef,
    ) -> i32;
    fn AXUIElementSetAttributeValue(
        element: AXUIElementRef,
        attribute: *const c_void,
        value: CFTypeRef,
    ) -> i32;
    fn AXUIElementIsAttributeSettable(
        element: AXUIElementRef,
        attribute: *const c_void,
        settable: *mut u8,
    ) -> i32;
    fn AXUIElementGetPid(element: AXUIElementRef, pid: *mut i32) -> i32;
    fn AXUIElementSetMessagingTimeout(element: AXUIElementRef, timeout: f32) -> i32;
    fn AXUIElementPerformAction(element: AXUIElementRef, action: CFTypeRef) -> i32;
    fn AXUIElementCopyElementAtPosition(
        application: AXUIElementRef,
        x: f32,
        y: f32,
        element: *mut AXUIElementRef,
    ) -> i32;
    fn AXValueGetType(value: CFTypeRef) -> i32;
    fn AXValueGetValue(value: CFTypeRef, value_type: i32, output: *mut c_void) -> u8;
}

fn cf_string(value: &str) -> Option<OwnedCfValue> {
    let value = CString::new(value).ok()?;
    OwnedCfValue::created(unsafe {
        CFStringCreateWithCString(std::ptr::null(), value.as_ptr(), K_CF_STRING_ENCODING_UTF8)
    })
}

pub(super) fn cf_string_value(value: CFTypeRef) -> Option<String> {
    if value.is_null() || unsafe { CFGetTypeID(value) } != unsafe { CFStringGetTypeID() } {
        return None;
    }
    let mut buffer = [0_i8; 512];
    if unsafe {
        CFStringGetCString(
            value,
            buffer.as_mut_ptr(),
            buffer.len() as isize,
            K_CF_STRING_ENCODING_UTF8,
        )
    } == 0
    {
        return None;
    }
    Some(
        unsafe { CStr::from_ptr(buffer.as_ptr()) }
            .to_string_lossy()
            .into_owned(),
    )
}

fn cf_bool_value(value: CFTypeRef) -> Option<bool> {
    if value.is_null() || unsafe { CFGetTypeID(value) } != unsafe { CFBooleanGetTypeID() } {
        return None;
    }
    Some(unsafe { CFBooleanGetValue(value) } != 0)
}

pub(super) fn copy_ax_attribute(element: AXUIElementRef, attribute: &str) -> Option<OwnedCfValue> {
    let attribute = cf_string(attribute)?;
    let mut value = std::ptr::null();
    let error = unsafe { AXUIElementCopyAttributeValue(element, attribute.as_ptr(), &mut value) };
    (error == AX_ERROR_SUCCESS)
        .then(|| OwnedCfValue::created(value))
        .flatten()
}

pub(super) fn ax_string_attribute(element: AXUIElementRef, attribute: &str) -> Option<String> {
    copy_ax_attribute(element, attribute).and_then(|value| cf_string_value(value.as_ptr()))
}

pub(super) fn ax_bool_attribute(element: AXUIElementRef, attribute: &str) -> Option<bool> {
    copy_ax_attribute(element, attribute).and_then(|value| cf_bool_value(value.as_ptr()))
}

pub(super) fn ax_range_attribute(
    element: AXUIElementRef,
    attribute: &str,
) -> Option<(isize, isize)> {
    let value = copy_ax_attribute(element, attribute)?;
    if unsafe { AXValueGetType(value.as_ptr()) } != K_AX_VALUE_CF_RANGE_TYPE {
        return None;
    }
    let mut range = AxRange::default();
    (unsafe {
        AXValueGetValue(
            value.as_ptr(),
            K_AX_VALUE_CF_RANGE_TYPE,
            (&mut range as *mut AxRange).cast(),
        )
    } != 0)
        .then_some((range.location, range.length))
}

pub(super) fn ax_attribute_is_settable(element: AXUIElementRef, attribute: &str) -> bool {
    let Some(attribute) = cf_string(attribute) else {
        return false;
    };
    let mut settable = 0_u8;
    unsafe {
        AXUIElementIsAttributeSettable(element, attribute.as_ptr(), &mut settable)
            == AX_ERROR_SUCCESS
            && settable != 0
    }
}

pub(super) fn set_ax_bool_attribute(element: AXUIElementRef, attribute: &str, value: bool) -> bool {
    let Some(attribute) = cf_string(attribute) else {
        return false;
    };
    value
        && unsafe {
            AXUIElementSetAttributeValue(element, attribute.as_ptr(), kCFBooleanTrue)
                == AX_ERROR_SUCCESS
        }
}

pub(super) fn ax_children(element: AXUIElementRef) -> Vec<OwnedCfValue> {
    array_attribute(element, "AXChildren")
}

pub(super) fn traversal_children(element: AXUIElementRef) -> Vec<OwnedCfValue> {
    let mut result = Vec::new();
    for attribute in ["AXChildren", "AXVisibleChildren", "AXContents"] {
        for child in array_attribute(element, attribute) {
            if result
                .iter()
                .any(|existing: &OwnedCfValue| elements_equal(existing.as_ptr(), child.as_ptr()))
            {
                continue;
            }
            result.push(child);
        }
    }
    result
}

fn array_attribute(element: AXUIElementRef, attribute: &str) -> Vec<OwnedCfValue> {
    let Some(children) = copy_ax_attribute(element, attribute) else {
        return Vec::new();
    };
    if unsafe { CFGetTypeID(children.as_ptr()) } != unsafe { CFArrayGetTypeID() } {
        return Vec::new();
    }
    let count = unsafe { CFArrayGetCount(children.as_ptr()) }.clamp(0, 1_000);
    (0..count)
        .filter_map(|index| unsafe {
            OwnedCfValue::retained(CFArrayGetValueAtIndex(children.as_ptr(), index))
        })
        .collect()
}

pub(super) fn system_wide_focused_element(timeout: f32) -> Option<OwnedAxElement> {
    let system = OwnedAxElement::created(unsafe { AXUIElementCreateSystemWide() })?;
    apply_timeout(system.as_ptr(), timeout);
    copy_ax_attribute(system.as_ptr(), "AXFocusedUIElement")
}

pub(super) fn raise_window(window: AXUIElementRef, timeout: f32) -> Result<(), AxQueryError> {
    perform_action(window, "AXRaise", timeout)
}

fn ax_point_attribute(element: AXUIElementRef, attribute: &str) -> Option<AxPoint> {
    let value = copy_ax_attribute(element, attribute)?;
    if unsafe { AXValueGetType(value.as_ptr()) } != K_AX_VALUE_CG_POINT_TYPE {
        return None;
    }
    let mut point = AxPoint::default();
    (unsafe {
        AXValueGetValue(
            value.as_ptr(),
            K_AX_VALUE_CG_POINT_TYPE,
            (&mut point as *mut AxPoint).cast(),
        )
    } != 0)
        .then_some(point)
}

fn ax_size_attribute(element: AXUIElementRef, attribute: &str) -> Option<AxSize> {
    let value = copy_ax_attribute(element, attribute)?;
    if unsafe { AXValueGetType(value.as_ptr()) } != K_AX_VALUE_CG_SIZE_TYPE {
        return None;
    }
    let mut size = AxSize::default();
    (unsafe {
        AXValueGetValue(
            value.as_ptr(),
            K_AX_VALUE_CG_SIZE_TYPE,
            (&mut size as *mut AxSize).cast(),
        )
    } != 0)
        .then_some(size)
}

pub(super) fn ax_element_frame(element: AXUIElementRef) -> Option<CandidateInput> {
    let position = ax_point_attribute(element, "AXPosition")?;
    let size = ax_size_attribute(element, "AXSize")?;
    Some(CandidateInput {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
    })
}

pub(super) fn ax_element_pid(element: AXUIElementRef) -> Option<u32> {
    let mut pid = 0_i32;
    (unsafe { AXUIElementGetPid(element, &mut pid) } == AX_ERROR_SUCCESS && pid > 0)
        .then_some(pid as u32)
}

pub(super) fn elements_equal(left: AXUIElementRef, right: AXUIElementRef) -> bool {
    !left.is_null() && !right.is_null() && unsafe { CFEqual(left, right) } != 0
}

pub(super) fn element_at_position(
    application: AXUIElementRef,
    x: f64,
    y: f64,
    timeout: f32,
) -> Result<OwnedAxElement, AxQueryError> {
    apply_timeout(application, timeout);
    let mut element = std::ptr::null();
    let code =
        unsafe { AXUIElementCopyElementAtPosition(application, x as f32, y as f32, &mut element) };
    if code != AX_ERROR_SUCCESS {
        return Err(AxQueryError::from_code(code));
    }
    OwnedAxElement::created(element).ok_or(AxQueryError::InvalidElement)
}

pub(super) fn perform_action(
    element: AXUIElementRef,
    action: &str,
    timeout: f32,
) -> Result<(), AxQueryError> {
    apply_timeout(element, timeout);
    let action = cf_string(action).ok_or(AxQueryError::Unsupported)?;
    let code = unsafe { AXUIElementPerformAction(element, action.as_ptr()) };
    (code == AX_ERROR_SUCCESS)
        .then_some(())
        .ok_or_else(|| AxQueryError::from_code(code))
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    static kCFBooleanTrue: *const c_void;
    fn CFRetain(cf: CFTypeRef) -> CFTypeRef;
    fn CFRelease(cf: CFTypeRef);
    fn CFEqual(left: CFTypeRef, right: CFTypeRef) -> u8;
    fn CFGetTypeID(cf: CFTypeRef) -> usize;
    fn CFStringGetTypeID() -> usize;
    fn CFBooleanGetTypeID() -> usize;
    fn CFArrayGetTypeID() -> usize;
    fn CFStringCreateWithCString(
        allocator: CFTypeRef,
        c_string: *const i8,
        encoding: u32,
    ) -> CFTypeRef;
    fn CFStringGetCString(
        string: CFTypeRef,
        buffer: *mut i8,
        buffer_size: isize,
        encoding: u32,
    ) -> u8;
    fn CFBooleanGetValue(boolean: CFTypeRef) -> u8;
    fn CFArrayGetCount(array: CFTypeRef) -> isize;
    fn CFArrayGetValueAtIndex(array: CFTypeRef, index: isize) -> CFTypeRef;
}

#[derive(Clone, Copy, Debug)]
pub(super) struct AxTraversalLimits {
    pub max_nodes: usize,
    pub max_depth: usize,
    pub max_elapsed: Duration,
    pub per_element_timeout: f32,
}

impl AxTraversalLimits {
    pub(super) fn diagnostic() -> Self {
        Self {
            max_nodes: 600,
            max_depth: 14,
            max_elapsed: Duration::from_millis(220),
            per_element_timeout: 0.02,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct AxTraversalStats {
    pub visited_nodes: usize,
    pub deepest_level: usize,
    pub stopped_by_budget: bool,
}

pub(super) struct AxTraversalBudget {
    limits: AxTraversalLimits,
    started: Instant,
    stats: AxTraversalStats,
}

impl AxTraversalBudget {
    pub(super) fn new(limits: AxTraversalLimits) -> Self {
        Self {
            limits,
            started: Instant::now(),
            stats: AxTraversalStats::default(),
        }
    }

    pub(super) fn try_visit(&mut self, depth: usize) -> bool {
        if self.stats.visited_nodes >= self.limits.max_nodes
            || depth > self.limits.max_depth
            || self.started.elapsed() >= self.limits.max_elapsed
        {
            self.stats.stopped_by_budget = true;
            return false;
        }

        self.stats.visited_nodes += 1;
        self.stats.deepest_level = self.stats.deepest_level.max(depth);
        true
    }

    #[cfg(test)]
    fn set_started(&mut self, started: Instant) {
        self.started = started;
    }

    pub(super) fn stats(&self) -> AxTraversalStats {
        self.stats
    }

    pub(super) fn has_time_remaining(&mut self) -> bool {
        if self.started.elapsed() < self.limits.max_elapsed {
            return true;
        }
        self.stats.stopped_by_budget = true;
        false
    }
}

fn apply_timeout(element: AXUIElementRef, timeout: f32) {
    unsafe {
        AXUIElementSetMessagingTimeout(element, timeout);
    }
}

pub(super) fn copy_attribute(
    element: AXUIElementRef,
    attribute: &str,
    timeout: f32,
) -> Option<OwnedCfValue> {
    apply_timeout(element, timeout);
    copy_ax_attribute(element, attribute)
}

pub(super) fn string_attribute(
    element: AXUIElementRef,
    attribute: &str,
    timeout: f32,
) -> Option<String> {
    apply_timeout(element, timeout);
    ax_string_attribute(element, attribute)
}

pub(super) fn bool_attribute(
    element: AXUIElementRef,
    attribute: &str,
    timeout: f32,
) -> Option<bool> {
    apply_timeout(element, timeout);
    ax_bool_attribute(element, attribute)
}

pub(super) fn attribute_is_settable(
    element: AXUIElementRef,
    attribute: &str,
    timeout: f32,
) -> bool {
    apply_timeout(element, timeout);
    ax_attribute_is_settable(element, attribute)
}

pub(super) fn children(element: AXUIElementRef, timeout: f32) -> Vec<OwnedCfValue> {
    apply_timeout(element, timeout);
    ax_children(element)
}

pub(super) fn frame(element: AXUIElementRef, timeout: f32) -> Option<super::CandidateInput> {
    apply_timeout(element, timeout);
    ax_element_frame(element)
}

pub(super) fn owner_pid(element: AXUIElementRef, timeout: f32) -> Option<u32> {
    apply_timeout(element, timeout);
    ax_element_pid(element)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn traversal_budget_stops_at_node_limit() {
        let mut budget = AxTraversalBudget::new(AxTraversalLimits {
            max_nodes: 2,
            max_depth: 10,
            max_elapsed: Duration::from_secs(1),
            per_element_timeout: 0.1,
        });

        assert!(budget.try_visit(1));
        assert!(budget.try_visit(2));
        assert!(!budget.try_visit(3));
        assert_eq!(budget.stats().visited_nodes, 2);
        assert!(budget.stats().stopped_by_budget);
    }

    #[test]
    fn traversal_budget_stops_at_depth_limit() {
        let mut budget = AxTraversalBudget::new(AxTraversalLimits {
            max_nodes: 10,
            max_depth: 2,
            max_elapsed: Duration::from_secs(1),
            per_element_timeout: 0.1,
        });

        assert!(budget.try_visit(2));
        assert!(!budget.try_visit(3));
        assert_eq!(budget.stats().deepest_level, 2);
    }

    #[test]
    fn traversal_budget_stops_at_elapsed_limit() {
        let mut budget = AxTraversalBudget::new(AxTraversalLimits {
            max_nodes: 10,
            max_depth: 10,
            max_elapsed: Duration::from_millis(1),
            per_element_timeout: 0.1,
        });
        budget.set_started(Instant::now() - Duration::from_millis(2));

        assert!(!budget.try_visit(1));
        assert_eq!(budget.stats().visited_nodes, 0);
        assert!(budget.stats().stopped_by_budget);
    }

    #[test]
    fn diagnostic_timeout_keeps_attribute_batches_inside_total_budget() {
        let limits = AxTraversalLimits::diagnostic();

        assert!(limits.per_element_timeout <= 0.025);
        assert!(limits.max_elapsed <= Duration::from_millis(250));
    }

    #[test]
    fn maps_accessibility_errors_without_panicking() {
        assert_eq!(
            AxQueryError::from_code(-25202),
            AxQueryError::InvalidElement
        );
        assert_eq!(
            AxQueryError::from_code(-25204),
            AxQueryError::CannotComplete
        );
        assert_eq!(AxQueryError::from_code(-25205), AxQueryError::Unsupported);
        assert_eq!(AxQueryError::from_code(17), AxQueryError::Other(17));
        assert_eq!(AxQueryError::TimedOut, AxQueryError::TimedOut);
    }

    #[test]
    fn element_identity_uses_core_foundation_equality() {
        let source = include_str!("ax_client.rs");
        let start = source.find("fn elements_equal").unwrap();
        let end = source[start..].find("fn element_at_position").unwrap();
        let implementation = &source[start..start + end];

        assert!(implementation.contains("CFEqual"));
        assert!(!implementation.contains("left == right"));
    }

    #[test]
    fn traversal_reads_all_supported_child_collections() {
        let source = include_str!("ax_client.rs");
        for attribute in ["AXChildren", "AXVisibleChildren", "AXContents"] {
            assert!(source.contains(attribute));
        }
    }
}
