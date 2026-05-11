//! OpenWhispr-style post-insertion text-field monitor.
//!
//! V1 only observes and emits/logs changes. Learning promotion comes later.

use tauri::AppHandle;

#[derive(Debug, Clone)]
pub struct EditMonitorSession {
    pub target_pid: Option<i32>,
    pub original_text: String,
    pub inserted_text: String,
}

pub fn start_after_insert(app: Option<AppHandle>, session: EditMonitorSession) {
    if !enabled() {
        log::info!("[edit-monitor] disabled; skip");
        return;
    }

    start_platform_monitor(app, session);
}

fn enabled() -> bool {
    std::env::var("OPENTYPELESS_EDIT_MONITOR_ENABLED")
        .map(|v| v != "0" && !v.eq_ignore_ascii_case("false"))
        .unwrap_or(true)
}

#[cfg(target_os = "macos")]
fn start_platform_monitor(app: Option<AppHandle>, session: EditMonitorSession) {
    let Some(app) = app else {
        log::info!("[edit-monitor] no app handle; skip");
        return;
    };
    let Some(target_pid) = session.target_pid else {
        log::info!("[edit-monitor] no target pid captured; skip");
        macos::write_skip_event(&session, "no_target_pid");
        return;
    };

    tauri::async_runtime::spawn(async move {
        if let Err(err) = macos::monitor(Some(app), target_pid, session).await {
            log::debug!("[edit-monitor] stopped: {err}");
        }
    });
}

#[cfg(not(target_os = "macos"))]
fn start_platform_monitor(_app: Option<AppHandle>, _session: EditMonitorSession) {
    log::debug!("[edit-monitor] target field monitor is only implemented on macOS for now");
}

#[cfg(target_os = "macos")]
pub fn capture_target_pid() -> Option<i32> {
    use objc2::msg_send;
    use objc2::runtime::{AnyClass, AnyObject};

    unsafe {
        let cls = AnyClass::get("NSWorkspace")?;
        let workspace: *mut AnyObject = msg_send![cls, sharedWorkspace];
        if workspace.is_null() {
            return None;
        }
        let app: *mut AnyObject = msg_send![workspace, frontmostApplication];
        if app.is_null() {
            return None;
        }
        let pid: i32 = msg_send![app, processIdentifier];
        if pid > 0 {
            Some(pid)
        } else {
            None
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub fn capture_target_pid() -> Option<i32> {
    None
}

#[cfg(target_os = "macos")]
pub async fn probe_current_focus(
    original_text: String,
    inserted_text: String,
) -> anyhow::Result<()> {
    let target_pid =
        capture_target_pid().ok_or_else(|| anyhow::anyhow!("no frontmost target pid"))?;
    probe_target_pid(target_pid, original_text, inserted_text).await
}

#[cfg(target_os = "macos")]
pub async fn probe_target_pid(
    target_pid: i32,
    original_text: String,
    inserted_text: String,
) -> anyhow::Result<()> {
    macos::monitor(
        None,
        target_pid,
        EditMonitorSession {
            target_pid: Some(target_pid),
            original_text,
            inserted_text,
        },
    )
    .await
}

#[cfg(not(target_os = "macos"))]
pub async fn probe_current_focus(
    _original_text: String,
    _inserted_text: String,
) -> anyhow::Result<()> {
    anyhow::bail!("edit monitor probe is only implemented on macOS")
}

#[cfg(target_os = "macos")]
mod macos {
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{mpsc, Mutex};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use serde_json::{json, Value};
    use tauri::{AppHandle, Emitter};

    use super::EditMonitorSession;

    const INITIAL_QUERY_DELAY_MS: u64 = 500;
    const INITIAL_QUERY_RETRIES: u8 = 4;
    const INITIAL_QUERY_RETRY_DELAY_MS: u64 = 300;
    const POLL_INTERVAL_MS: u64 = 500;
    const MONITOR_TIMEOUT_MS: u64 = 30_000;

    static ACTIVE_MONITOR_GENERATION: AtomicU64 = AtomicU64::new(0);
    static JSONL_WRITE_LOCK: Mutex<()> = Mutex::new(());

    pub async fn monitor(
        app: Option<AppHandle>,
        target_pid: i32,
        session: EditMonitorSession,
    ) -> anyhow::Result<()> {
        let generation = next_monitor_generation();
        log::info!(
            "[edit-monitor] start generation={} target_pid={} inserted_chars={}",
            generation,
            target_pid,
            session.inserted_text.chars().count()
        );
        write_event(
            "monitor_start",
            &session,
            target_pid,
            json!({
                "generation": generation,
                "insertedChars": session.inserted_text.chars().count(),
            }),
        );

        let observer_app = app.clone();
        let observer_session = session.clone();
        match tokio::task::spawn_blocking(move || {
            observer_poll_monitor(observer_app, target_pid, observer_session, generation)
        })
        .await
        {
            Ok(Ok(())) => return Ok(()),
            Ok(Err(err)) => {
                log::debug!("[edit-monitor] observer monitor unavailable: {err}; using polling");
            }
            Err(err) => {
                log::debug!("[edit-monitor] observer task failed: {err}; using polling");
            }
        }

        poll_monitor(app, target_pid, session, generation).await
    }

    fn next_monitor_generation() -> u64 {
        ACTIVE_MONITOR_GENERATION.fetch_add(1, Ordering::SeqCst) + 1
    }

    fn is_current_generation(generation: u64) -> bool {
        ACTIVE_MONITOR_GENERATION.load(Ordering::SeqCst) == generation
    }

    fn observer_poll_monitor(
        app: Option<AppHandle>,
        target_pid: i32,
        session: EditMonitorSession,
        generation: u64,
    ) -> anyhow::Result<()> {
        let _ = enable_ax_enhanced_ui(target_pid);
        std::thread::sleep(Duration::from_millis(INITIAL_QUERY_DELAY_MS));
        if stop_if_superseded(generation, &session, target_pid) {
            return Ok(());
        }

        let (focused, value) = ax::focused_text_element_for_app(target_pid)?;
        log::info!(
            "[edit-monitor] initial field value chars={}",
            value.chars().count()
        );
        write_event(
            "initial_value",
            &session,
            target_pid,
            json!({
                "generation": generation,
                "fieldChars": value.chars().count(),
                "fieldValue": value,
            }),
        );
        run_observer_loop(app, target_pid, session, generation, focused, value)
    }

    fn run_observer_loop(
        app: Option<AppHandle>,
        target_pid: i32,
        session: EditMonitorSession,
        generation: u64,
        focused: ax::AxUiElementRef,
        mut last_value: String,
    ) -> anyhow::Result<()> {
        let (tx, rx) = mpsc::channel();
        let observer = match unsafe { ax::create_value_observer(target_pid, focused, tx) } {
            Ok(observer) => observer,
            Err(err) => {
                unsafe {
                    ax::release_element(focused);
                }
                return Err(err);
            }
        };
        let started = std::time::Instant::now();
        let mut last_poll = std::time::Instant::now();
        let initial_value = last_value.clone();
        let mut observed_change = false;

        loop {
            if stop_if_superseded(generation, &session, target_pid) {
                unsafe {
                    ax::release_observer(observer);
                    ax::release_element(focused);
                }
                return Ok(());
            }

            if started.elapsed() >= Duration::from_millis(MONITOR_TIMEOUT_MS) {
                log::debug!("[edit-monitor] timeout");
                record_final_candidate_if_changed(
                    &session,
                    target_pid,
                    &initial_value,
                    &last_value,
                    observed_change,
                    generation,
                );
                write_event("timeout", &session, target_pid, json!({}));
                unsafe {
                    ax::release_observer(observer);
                    ax::release_element(focused);
                }
                return Ok(());
            }

            unsafe {
                ax::run_observer_slice(0.1);
            }

            while let Ok(current) = rx.try_recv() {
                if current != last_value {
                    last_value = current.clone();
                    observed_change = true;
                    emit_change(app.as_ref(), &session, target_pid, current, generation);
                }
            }

            if last_poll.elapsed() >= Duration::from_millis(POLL_INTERVAL_MS) {
                last_poll = std::time::Instant::now();
                match query_focused_value(target_pid)? {
                    Some(current) => {
                        if current != last_value {
                            last_value = current.clone();
                            observed_change = true;
                            emit_change(app.as_ref(), &session, target_pid, current, generation);
                        }
                    }
                    None => {
                        log::debug!("[edit-monitor] target field disappeared");
                        record_final_candidate_if_changed(
                            &session,
                            target_pid,
                            &initial_value,
                            &last_value,
                            observed_change,
                            generation,
                        );
                        write_event("target_field_disappeared", &session, target_pid, json!({}));
                        unsafe {
                            ax::release_observer(observer);
                            ax::release_element(focused);
                        }
                        return Ok(());
                    }
                }
            }
        }
    }

    async fn poll_monitor(
        app: Option<AppHandle>,
        target_pid: i32,
        session: EditMonitorSession,
        generation: u64,
    ) -> anyhow::Result<()> {
        let _ = enable_ax_enhanced_ui(target_pid);
        tokio::time::sleep(Duration::from_millis(INITIAL_QUERY_DELAY_MS)).await;
        if stop_if_superseded(generation, &session, target_pid) {
            return Ok(());
        }

        let mut last_value = match initial_value(target_pid).await {
            Ok(value) => {
                log::info!(
                    "[edit-monitor] initial field value chars={}",
                    value.chars().count()
                );
                write_event(
                    "initial_value",
                    &session,
                    target_pid,
                    json!({
                        "generation": generation,
                        "fieldChars": value.chars().count(),
                        "fieldValue": value,
                    }),
                );
                value
            }
            Err(err) => {
                write_event(
                    "initial_query_failed",
                    &session,
                    target_pid,
                    json!({
                        "error": err.to_string(),
                    }),
                );
                return Err(err);
            }
        };
        let started = tokio::time::Instant::now();
        let initial_value = last_value.clone();
        let mut observed_change = false;

        loop {
            if stop_if_superseded(generation, &session, target_pid) {
                return Ok(());
            }

            if started.elapsed() >= Duration::from_millis(MONITOR_TIMEOUT_MS) {
                log::debug!("[edit-monitor] timeout");
                record_final_candidate_if_changed(
                    &session,
                    target_pid,
                    &initial_value,
                    &last_value,
                    observed_change,
                    generation,
                );
                write_event("timeout", &session, target_pid, json!({}));
                return Ok(());
            }

            tokio::time::sleep(Duration::from_millis(POLL_INTERVAL_MS)).await;
            if stop_if_superseded(generation, &session, target_pid) {
                return Ok(());
            }
            let current = match query_focused_value(target_pid)? {
                Some(value) => value,
                None => {
                    log::debug!("[edit-monitor] target field disappeared");
                    record_final_candidate_if_changed(
                        &session,
                        target_pid,
                        &initial_value,
                        &last_value,
                        observed_change,
                        generation,
                    );
                    write_event("target_field_disappeared", &session, target_pid, json!({}));
                    return Ok(());
                }
            };

            if current != last_value {
                last_value = current.clone();
                observed_change = true;
                emit_change(app.as_ref(), &session, target_pid, current, generation);
            }
        }
    }

    fn stop_if_superseded(generation: u64, session: &EditMonitorSession, target_pid: i32) -> bool {
        if is_current_generation(generation) {
            return false;
        }

        let active_generation = ACTIVE_MONITOR_GENERATION.load(Ordering::SeqCst);
        log::debug!(
            "[edit-monitor] superseded generation={} active={}",
            generation,
            active_generation
        );
        write_event(
            "monitor_superseded",
            session,
            target_pid,
            json!({
                "generation": generation,
                "activeGeneration": active_generation,
            }),
        );
        true
    }

    fn emit_change(
        app: Option<&AppHandle>,
        session: &EditMonitorSession,
        target_pid: i32,
        current: String,
        generation: u64,
    ) {
        if !is_current_generation(generation) {
            return;
        }

        log::info!(
            "[edit-monitor] target field changed chars={}",
            current.chars().count()
        );
        if let Some(app) = app {
            let _ = app.emit(
                "edit-monitor:text-edited",
                json!({
                    "originalText": session.original_text.clone(),
                    "insertedText": session.inserted_text.clone(),
                    "newFieldValue": current,
                    "targetPid": target_pid,
                }),
            );
        }
        write_observation(session, target_pid, &current);
    }

    fn record_final_candidate_if_changed(
        session: &EditMonitorSession,
        target_pid: i32,
        initial_value: &str,
        final_value: &str,
        observed_change: bool,
        generation: u64,
    ) {
        if !observed_change {
            return;
        }
        if !is_current_generation(generation) {
            return;
        }

        crate::learning_probe::record_final_candidate(
            session,
            target_pid,
            initial_value,
            final_value,
        );
    }

    pub fn write_skip_event(session: &EditMonitorSession, reason: &str) {
        write_jsonl(json!({
            "event": "monitor_skipped",
            "timestampMs": timestamp_ms(),
            "targetPid": null,
            "originalText": session.original_text,
            "insertedText": session.inserted_text,
            "details": {
                "reason": reason,
            },
        }));
    }

    async fn initial_value(target_pid: i32) -> anyhow::Result<String> {
        for attempt in 1..=INITIAL_QUERY_RETRIES {
            match query_focused_value(target_pid)? {
                Some(value) if !value.is_empty() => return Ok(value),
                Some(_) if attempt < INITIAL_QUERY_RETRIES => {
                    tokio::time::sleep(Duration::from_millis(INITIAL_QUERY_RETRY_DELAY_MS)).await;
                }
                Some(value) => return Ok(value),
                None if attempt < INITIAL_QUERY_RETRIES => {
                    tokio::time::sleep(Duration::from_millis(INITIAL_QUERY_RETRY_DELAY_MS)).await;
                }
                None => anyhow::bail!("no focused target field"),
            }
        }
        Ok(String::new())
    }

    fn enable_ax_enhanced_ui(target_pid: i32) -> anyhow::Result<()> {
        ax::set_enhanced_user_interface(target_pid)
    }

    fn query_focused_value(target_pid: i32) -> anyhow::Result<Option<String>> {
        ax::focused_value(target_pid)
    }

    fn write_event(event: &str, session: &EditMonitorSession, target_pid: i32, details: Value) {
        write_jsonl(json!({
            "event": event,
            "timestampMs": timestamp_ms(),
            "targetPid": target_pid,
            "originalText": session.original_text,
            "insertedText": session.inserted_text,
            "details": details,
        }));
    }

    fn write_observation(session: &EditMonitorSession, target_pid: i32, new_field_value: &str) {
        write_jsonl(json!({
            "event": "text_changed",
            "timestampMs": timestamp_ms(),
            "targetPid": target_pid,
            "originalText": session.original_text,
            "insertedText": session.inserted_text,
            "newFieldValue": new_field_value,
        }));
    }

    fn write_jsonl(payload: Value) {
        if !jsonl_enabled() {
            return;
        }

        let Some(path) = jsonl_path() else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let _guard = JSONL_WRITE_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
            Ok(mut file) => {
                let line = format!("{payload}\n");
                match file.write_all(line.as_bytes()) {
                    Ok(()) => log::info!("[edit-monitor] wrote observation to {}", path.display()),
                    Err(err) => log::debug!("[edit-monitor] failed to write observation: {err}"),
                }
            }
            Err(err) => {
                log::debug!("[edit-monitor] failed to write observation: {err}");
            }
        }
    }

    fn timestamp_ms() -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or_default()
    }

    fn jsonl_enabled() -> bool {
        std::env::var("OPENTYPELESS_EDIT_MONITOR_JSONL")
            .map(|v| v != "0" && !v.eq_ignore_ascii_case("false"))
            .unwrap_or(true)
    }

    fn jsonl_path() -> Option<PathBuf> {
        std::env::var_os("OPENTYPELESS_EDIT_MONITOR_JSONL_PATH")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME").map(PathBuf::from).map(|home| {
                    home.join(".openless")
                        .join("opentypeless-edit-monitor.jsonl")
                })
            })
    }

    mod ax {
        use std::ffi::{c_void, CStr};
        use std::os::raw::{c_char, c_int};
        use std::sync::mpsc::Sender;
        use std::time::Duration;

        pub(super) type AxUiElementRef = *mut c_void;
        type AxObserverRef = *mut c_void;
        type CFRunLoopRef = *mut c_void;
        type CFRunLoopSourceRef = *const c_void;
        type CFStringRef = *const c_void;
        type CFTypeRef = *const c_void;
        type CFAllocatorRef = *const c_void;
        type CFBooleanRef = *const c_void;
        type CFTypeId = usize;
        type CFIndex = isize;
        type AxError = i32;
        type AxObserverCallback =
            extern "C" fn(AxObserverRef, AxUiElementRef, CFStringRef, *mut c_void);

        const AX_ERROR_SUCCESS: AxError = 0;
        const K_CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
        const K_AX_VALUE_CF_RANGE_TYPE: u32 = 4;
        const K_CF_NUMBER_CF_INDEX_TYPE: CFIndex = 14;
        const MAX_DESCENDANT_DEPTH: u8 = 6;
        const MAX_DESCENDANT_NODES: u16 = 128;
        const MAX_RANGE_TEXT_CHARS: CFIndex = 10_000;

        pub(super) struct ValueObserver {
            observer: AxObserverRef,
            context: *mut ObserverContext,
        }

        struct ObserverContext {
            tx: Sender<String>,
        }

        #[link(name = "ApplicationServices", kind = "framework")]
        extern "C" {
            fn AXUIElementCreateApplication(pid: c_int) -> AxUiElementRef;
            fn AXUIElementCreateSystemWide() -> AxUiElementRef;
            fn AXUIElementCopyAttributeValue(
                element: AxUiElementRef,
                attribute: CFStringRef,
                value: *mut CFTypeRef,
            ) -> AxError;
            fn AXUIElementCopyParameterizedAttributeValue(
                element: AxUiElementRef,
                parameterized_attribute: CFStringRef,
                parameter: CFTypeRef,
                value: *mut CFTypeRef,
            ) -> AxError;
            fn AXUIElementGetPid(element: AxUiElementRef, pid: *mut c_int) -> AxError;
            fn AXUIElementSetAttributeValue(
                element: AxUiElementRef,
                attribute: CFStringRef,
                value: CFTypeRef,
            ) -> AxError;
            fn AXObserverCreate(
                application: c_int,
                callback: AxObserverCallback,
                observer: *mut AxObserverRef,
            ) -> AxError;
            fn AXObserverAddNotification(
                observer: AxObserverRef,
                element: AxUiElementRef,
                notification: CFStringRef,
                refcon: *mut c_void,
            ) -> AxError;
            fn AXObserverGetRunLoopSource(observer: AxObserverRef) -> CFRunLoopSourceRef;
        }

        #[link(name = "CoreFoundation", kind = "framework")]
        extern "C" {
            static kCFBooleanTrue: CFBooleanRef;
            static kCFRunLoopDefaultMode: CFStringRef;

            fn CFGetTypeID(cf: CFTypeRef) -> CFTypeId;
            fn CFNumberGetValue(number: CFTypeRef, the_type: CFIndex, value: *mut c_void) -> bool;
            fn CFRelease(cf: CFTypeRef);
            fn CFRunLoopAddSource(rl: CFRunLoopRef, source: CFRunLoopSourceRef, mode: CFStringRef);
            fn CFRunLoopGetCurrent() -> CFRunLoopRef;
            fn CFRunLoopRunInMode(
                mode: CFStringRef,
                seconds: f64,
                return_after_source_handled: u8,
            ) -> i32;
            fn CFStringCreateWithCString(
                allocator: CFAllocatorRef,
                cstr: *const c_char,
                encoding: u32,
            ) -> CFStringRef;
            fn CFStringGetTypeID() -> CFTypeId;
            fn CFStringGetCStringPtr(s: CFStringRef, encoding: u32) -> *const c_char;
            fn CFStringGetCString(
                s: CFStringRef,
                buffer: *mut c_char,
                buffer_size: isize,
                encoding: u32,
            ) -> bool;
            fn CFStringGetLength(s: CFStringRef) -> isize;
            fn CFStringGetMaximumSizeForEncoding(length: isize, encoding: u32) -> isize;
            fn CFArrayGetTypeID() -> CFTypeId;
            fn CFArrayGetCount(array: CFTypeRef) -> CFIndex;
            fn CFArrayGetValueAtIndex(array: CFTypeRef, idx: CFIndex) -> *const c_void;
            fn AXValueCreate(the_type: u32, value: *const c_void) -> CFTypeRef;
        }

        pub fn set_enhanced_user_interface(target_pid: i32) -> anyhow::Result<()> {
            unsafe {
                let app = AXUIElementCreateApplication(target_pid as c_int);
                if app.is_null() {
                    anyhow::bail!("AXUIElementCreateApplication returned null");
                }
                let attr = cfstring_from_static(b"AXEnhancedUserInterface\0")
                    .ok_or_else(|| anyhow::anyhow!("failed to create AX attr"))?;
                let err = AXUIElementSetAttributeValue(app, attr, kCFBooleanTrue as CFTypeRef);
                CFRelease(app as CFTypeRef);
                CFRelease(attr);
                if err != AX_ERROR_SUCCESS {
                    log::debug!(
                        "[edit-monitor] AXEnhancedUserInterface failed pid={} err={}",
                        target_pid,
                        err
                    );
                    anyhow::bail!("AXUIElementSetAttributeValue failed: {err}");
                }
                Ok(())
            }
        }

        pub fn focused_value(target_pid: i32) -> anyhow::Result<Option<String>> {
            unsafe {
                let focused = focused_element_for_app(target_pid)?
                    .or_else(|| focused_element_system_wide(target_pid).ok().flatten());
                if let Some(focused) = focused {
                    let value = copy_text_value(focused)
                        .filter(|value| !value.is_empty())
                        .or_else(|| {
                            let mut visited = 0;
                            first_descendant_text_value(focused, 0, &mut visited)
                        });
                    CFRelease(focused as CFTypeRef);
                    if value.is_some() {
                        return Ok(value);
                    }
                    return Ok(Some(String::new()));
                }

                focused_window_text_for_app(target_pid)
            }
        }

        pub fn focused_text_element_for_app(
            target_pid: i32,
        ) -> anyhow::Result<(AxUiElementRef, String)> {
            const MAX_RETRIES: u8 = 5;
            for attempt in 1..=MAX_RETRIES {
                unsafe {
                    match focused_element_for_app(target_pid)? {
                        Some(focused) => {
                            if let Some(value) = copy_text_value(focused) {
                                if attempt > 1 {
                                    log::debug!(
                                        "[edit-monitor] got focused text element on attempt {}",
                                        attempt
                                    );
                                }
                                return Ok((focused, value));
                            }
                            CFRelease(focused as CFTypeRef);
                        }
                        None => {
                            log::debug!(
                                "[edit-monitor] focused text element unavailable attempt={}",
                                attempt
                            );
                        }
                    }
                }
                if attempt < MAX_RETRIES {
                    std::thread::sleep(Duration::from_millis(300));
                }
            }
            anyhow::bail!("focused element has no text value");
        }

        pub unsafe fn create_value_observer(
            target_pid: i32,
            element: AxUiElementRef,
            tx: Sender<String>,
        ) -> anyhow::Result<ValueObserver> {
            let mut observer: AxObserverRef = std::ptr::null_mut();
            let err = AXObserverCreate(target_pid as c_int, observer_callback, &mut observer);
            if err != AX_ERROR_SUCCESS || observer.is_null() {
                anyhow::bail!("AXObserverCreate failed: {err}");
            }

            let notification = cfstring_from_static(b"AXValueChanged\0")
                .ok_or_else(|| anyhow::anyhow!("failed to create AXValueChanged"))?;
            let context = Box::into_raw(Box::new(ObserverContext { tx }));
            let add_err =
                AXObserverAddNotification(observer, element, notification, context as *mut c_void);
            CFRelease(notification);
            if add_err != AX_ERROR_SUCCESS {
                drop(Box::from_raw(context));
                CFRelease(observer as CFTypeRef);
                anyhow::bail!("AXObserverAddNotification failed: {add_err}");
            }

            CFRunLoopAddSource(
                CFRunLoopGetCurrent(),
                AXObserverGetRunLoopSource(observer),
                kCFRunLoopDefaultMode,
            );
            Ok(ValueObserver { observer, context })
        }

        pub unsafe fn run_observer_slice(seconds: f64) {
            let _ = CFRunLoopRunInMode(kCFRunLoopDefaultMode, seconds, 1);
        }

        pub unsafe fn release_observer(observer: ValueObserver) {
            CFRelease(observer.observer as CFTypeRef);
            drop(Box::from_raw(observer.context));
        }

        pub unsafe fn release_element(element: AxUiElementRef) {
            CFRelease(element as CFTypeRef);
        }

        extern "C" fn observer_callback(
            _observer: AxObserverRef,
            element: AxUiElementRef,
            _notification: CFStringRef,
            refcon: *mut c_void,
        ) {
            if refcon.is_null() {
                return;
            }
            unsafe {
                let context = &*(refcon as *const ObserverContext);
                if let Some(value) = copy_text_value(element) {
                    let _ = context.tx.send(value);
                }
            }
        }

        unsafe fn focused_element_for_app(
            target_pid: i32,
        ) -> anyhow::Result<Option<AxUiElementRef>> {
            let app = AXUIElementCreateApplication(target_pid as c_int);
            if app.is_null() {
                anyhow::bail!("AXUIElementCreateApplication returned null");
            }

            let focused = copy_focused_element(app, target_pid, "app")?;
            CFRelease(app as CFTypeRef);
            Ok(focused)
        }

        unsafe fn focused_element_system_wide(
            target_pid: i32,
        ) -> anyhow::Result<Option<AxUiElementRef>> {
            let system = AXUIElementCreateSystemWide();
            if system.is_null() {
                anyhow::bail!("AXUIElementCreateSystemWide returned null");
            }

            let focused = copy_focused_element(system, target_pid, "system-wide")?;
            CFRelease(system as CFTypeRef);
            let Some(focused) = focused else {
                return Ok(None);
            };

            let mut focused_pid: c_int = 0;
            let err = AXUIElementGetPid(focused, &mut focused_pid);
            if err != AX_ERROR_SUCCESS || focused_pid != target_pid as c_int {
                log::debug!(
                    "[edit-monitor] system-wide focused element pid mismatch target={} focused={} err={}",
                    target_pid,
                    focused_pid,
                    err
                );
                CFRelease(focused as CFTypeRef);
                return Ok(None);
            }

            Ok(Some(focused))
        }

        unsafe fn focused_window_text_for_app(target_pid: i32) -> anyhow::Result<Option<String>> {
            let app = AXUIElementCreateApplication(target_pid as c_int);
            if app.is_null() {
                anyhow::bail!("AXUIElementCreateApplication returned null");
            }

            let window_attr = cfstring_from_static(b"AXFocusedWindow\0")
                .ok_or_else(|| anyhow::anyhow!("failed to create AX focused window attr"))?;
            let mut window: CFTypeRef = std::ptr::null();
            let err = AXUIElementCopyAttributeValue(app, window_attr, &mut window);
            CFRelease(window_attr);
            CFRelease(app as CFTypeRef);
            if err != AX_ERROR_SUCCESS || window.is_null() {
                log::debug!(
                    "[edit-monitor] AXFocusedWindow unavailable pid={} err={}",
                    target_pid,
                    err
                );
                return Ok(None);
            }

            let mut visited = 0;
            let value = first_descendant_text_value(window as AxUiElementRef, 0, &mut visited);
            CFRelease(window);
            Ok(value)
        }

        unsafe fn copy_focused_element(
            element: AxUiElementRef,
            target_pid: i32,
            source: &str,
        ) -> anyhow::Result<Option<AxUiElementRef>> {
            let focused_attr = cfstring_from_static(b"AXFocusedUIElement\0")
                .ok_or_else(|| anyhow::anyhow!("failed to create AX focus attr"))?;
            let mut focused: CFTypeRef = std::ptr::null();
            let err = AXUIElementCopyAttributeValue(element, focused_attr, &mut focused);
            CFRelease(focused_attr);
            if err != AX_ERROR_SUCCESS || focused.is_null() {
                log::debug!(
                    "[edit-monitor] AXFocusedUIElement unavailable source={} pid={} err={}",
                    source,
                    target_pid,
                    err
                );
                return Ok(None);
            }

            Ok(Some(focused as AxUiElementRef))
        }

        unsafe fn first_descendant_text_value(
            element: AxUiElementRef,
            depth: u8,
            visited: &mut u16,
        ) -> Option<String> {
            if depth >= MAX_DESCENDANT_DEPTH || *visited >= MAX_DESCENDANT_NODES {
                return None;
            }
            *visited += 1;

            if let Some(value) = copy_text_value(element).filter(|value| !value.is_empty()) {
                return Some(value);
            }

            let children = copy_array_attribute(element, b"AXChildren\0")?;
            let count = CFArrayGetCount(children);
            for idx in 0..count {
                let child = CFArrayGetValueAtIndex(children, idx) as AxUiElementRef;
                if child.is_null() {
                    continue;
                }
                if let Some(value) = first_descendant_text_value(child, depth + 1, visited) {
                    CFRelease(children);
                    return Some(value);
                }
            }
            CFRelease(children);
            None
        }

        unsafe fn copy_text_value(element: AxUiElementRef) -> Option<String> {
            let value = copy_string_attribute(element, b"AXValue\0");
            if value.as_ref().is_some_and(|value| !value.is_empty()) {
                return value;
            }

            let range_value = copy_string_for_full_range(element);
            if range_value.as_ref().is_some_and(|value| !value.is_empty()) {
                return range_value;
            }

            let selected_text = copy_string_attribute(element, b"AXSelectedText\0");
            if selected_text
                .as_ref()
                .is_some_and(|value| !value.is_empty())
            {
                return selected_text;
            }

            value.or(range_value).or(selected_text)
        }

        #[repr(C)]
        struct CfRange {
            location: CFIndex,
            length: CFIndex,
        }

        unsafe fn copy_string_for_full_range(element: AxUiElementRef) -> Option<String> {
            let char_count = copy_cfindex_attribute(element, b"AXNumberOfCharacters\0")?;
            if char_count <= 0 {
                return None;
            }

            let attr = cfstring_from_static(b"AXStringForRange\0")?;
            let range = CfRange {
                location: 0,
                length: char_count.min(MAX_RANGE_TEXT_CHARS),
            };
            let parameter = AXValueCreate(
                K_AX_VALUE_CF_RANGE_TYPE,
                &range as *const CfRange as *const c_void,
            );
            if parameter.is_null() {
                CFRelease(attr);
                return None;
            }

            let mut value: CFTypeRef = std::ptr::null();
            let err =
                AXUIElementCopyParameterizedAttributeValue(element, attr, parameter, &mut value);
            CFRelease(parameter);
            CFRelease(attr);
            if err != AX_ERROR_SUCCESS || value.is_null() {
                return None;
            }

            let result = cfstring_to_rust(value);
            CFRelease(value);
            result
        }

        unsafe fn copy_string_attribute(
            element: AxUiElementRef,
            attr_name: &[u8],
        ) -> Option<String> {
            let attr = cfstring_from_static(attr_name)?;
            let mut value: CFTypeRef = std::ptr::null();
            let err = AXUIElementCopyAttributeValue(element, attr, &mut value);
            CFRelease(attr);
            if err != AX_ERROR_SUCCESS || value.is_null() {
                return None;
            }
            let result = cfstring_to_rust(value);
            CFRelease(value);
            result
        }

        unsafe fn copy_cfindex_attribute(
            element: AxUiElementRef,
            attr_name: &[u8],
        ) -> Option<CFIndex> {
            let attr = cfstring_from_static(attr_name)?;
            let mut value: CFTypeRef = std::ptr::null();
            let err = AXUIElementCopyAttributeValue(element, attr, &mut value);
            CFRelease(attr);
            if err != AX_ERROR_SUCCESS || value.is_null() {
                return None;
            }

            let mut number: CFIndex = 0;
            let ok = CFNumberGetValue(
                value,
                K_CF_NUMBER_CF_INDEX_TYPE,
                &mut number as *mut CFIndex as *mut c_void,
            );
            CFRelease(value);
            if ok {
                Some(number)
            } else {
                None
            }
        }

        unsafe fn copy_array_attribute(
            element: AxUiElementRef,
            attr_name: &[u8],
        ) -> Option<CFTypeRef> {
            let attr = cfstring_from_static(attr_name)?;
            let mut value: CFTypeRef = std::ptr::null();
            let err = AXUIElementCopyAttributeValue(element, attr, &mut value);
            CFRelease(attr);
            if err != AX_ERROR_SUCCESS || value.is_null() {
                return None;
            }
            if CFGetTypeID(value) != CFArrayGetTypeID() {
                CFRelease(value);
                return None;
            }
            Some(value)
        }

        unsafe fn cfstring_from_static(bytes_with_nul: &[u8]) -> Option<CFStringRef> {
            let cstr = CStr::from_bytes_with_nul(bytes_with_nul).ok()?;
            let s = CFStringCreateWithCString(
                std::ptr::null(),
                cstr.as_ptr(),
                K_CF_STRING_ENCODING_UTF8,
            );
            if s.is_null() {
                None
            } else {
                Some(s)
            }
        }

        unsafe fn cfstring_to_rust(value: CFTypeRef) -> Option<String> {
            if CFGetTypeID(value) != CFStringGetTypeID() {
                return None;
            }
            let s = value as CFStringRef;
            let direct = CFStringGetCStringPtr(s, K_CF_STRING_ENCODING_UTF8);
            if !direct.is_null() {
                let cstr = CStr::from_ptr(direct);
                return cstr.to_str().ok().map(|s| s.to_string());
            }
            let length = CFStringGetLength(s);
            if length <= 0 {
                return Some(String::new());
            }
            let max_bytes =
                CFStringGetMaximumSizeForEncoding(length, K_CF_STRING_ENCODING_UTF8) + 1;
            let mut buf: Vec<u8> = vec![0; max_bytes as usize];
            let ok = CFStringGetCString(
                s,
                buf.as_mut_ptr() as *mut c_char,
                max_bytes,
                K_CF_STRING_ENCODING_UTF8,
            );
            if !ok {
                return None;
            }
            let cstr = CStr::from_ptr(buf.as_ptr() as *const c_char);
            cstr.to_str().ok().map(|s| s.to_string())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

        #[test]
        fn latest_monitor_generation_supersedes_previous() {
            let previous = next_monitor_generation();
            let latest = next_monitor_generation();

            assert!(!is_current_generation(previous));
            assert!(is_current_generation(latest));
        }

        #[test]
        fn write_observation_appends_jsonl_payload() {
            let _guard = ENV_LOCK.lock().unwrap();
            let path = std::env::temp_dir().join(format!(
                "opentypeless-edit-monitor-{}-{}.jsonl",
                std::process::id(),
                "payload"
            ));
            let _ = std::fs::remove_file(&path);
            std::env::set_var("OPENTYPELESS_EDIT_MONITOR_JSONL_PATH", &path);
            std::env::remove_var("OPENTYPELESS_EDIT_MONITOR_JSONL");

            let session = EditMonitorSession {
                target_pid: Some(42),
                original_text: "open claw".into(),
                inserted_text: "OpenClaw".into(),
            };
            write_observation(&session, 42, "OpenClaw edited");

            let contents = std::fs::read_to_string(&path).unwrap();
            let value: serde_json::Value = serde_json::from_str(contents.trim()).unwrap();
            assert_eq!(value["targetPid"], 42);
            assert_eq!(value["originalText"], "open claw");
            assert_eq!(value["insertedText"], "OpenClaw");
            assert_eq!(value["newFieldValue"], "OpenClaw edited");

            let _ = std::fs::remove_file(&path);
            std::env::remove_var("OPENTYPELESS_EDIT_MONITOR_JSONL_PATH");
        }

        #[test]
        fn write_observation_respects_jsonl_disable_flag() {
            let _guard = ENV_LOCK.lock().unwrap();
            let path = std::env::temp_dir().join(format!(
                "opentypeless-edit-monitor-{}-{}.jsonl",
                std::process::id(),
                "disabled"
            ));
            let _ = std::fs::remove_file(&path);
            std::env::set_var("OPENTYPELESS_EDIT_MONITOR_JSONL_PATH", &path);
            std::env::set_var("OPENTYPELESS_EDIT_MONITOR_JSONL", "0");

            let session = EditMonitorSession {
                target_pid: Some(42),
                original_text: "raw".into(),
                inserted_text: "polished".into(),
            };
            write_observation(&session, 42, "edited");

            assert!(!path.exists());

            std::env::remove_var("OPENTYPELESS_EDIT_MONITOR_JSONL");
            std::env::remove_var("OPENTYPELESS_EDIT_MONITOR_JSONL_PATH");
        }
    }
}
