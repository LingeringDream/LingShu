// ── L3 Screen Reading (macOS Accessibility) ─────────────────────────────
//
// Reads on-screen text from the frontmost non-LingShu window via the macOS
// Accessibility C API (HIServices), IN-PROCESS in the main Tauri app.
//
// Why in the main app and not the sidecar: TCC attributes the Accessibility
// grant to the *responsible process* identified by its code signature. The
// bundled `lingshu-server` sidecar is a bare Mach-O binary with an ad-hoc,
// per-build identity (`lingshu-server-<cdhash>`) and is re-parented to launchd
// — so it is its own TCC subject and never matches the 「灵枢 LingShu」 entry the
// user checks in the list. The Tauri binary *is* `com.lingshu.desktop`, the
// exact subject the user authorizes, so the AX call must originate here.

/// Tauri command: read visible text from the frontmost application window
/// (skipping LingShu's own windows). Runs in the main app process so the
/// Accessibility grant attributed to `com.lingshu.desktop` actually applies.
#[tauri::command]
pub fn read_screen() -> Result<String, String> {
    #[cfg(target_os = "macos")]
    {
        // Run the AX walk on a dedicated thread, NOT the Tauri main thread.
        // AX queries do synchronous IPC that pumps the run loop; doing that
        // while the main thread is inside WebKit's command dispatch can
        // re-enter and crash. A worker thread also keeps the UI responsive
        // during the (bounded) walk.
        std::thread::Builder::new()
            .name("lingshu-screenread".into())
            .spawn(|| unsafe { read_frontmost() })
            .map_err(|e| format!("无法启动屏幕读取线程：{e}"))?
            .join()
            .unwrap_or_else(|_| Err("屏幕读取线程异常退出（已避免崩溃）".into()))
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("屏幕阅读仅在 macOS 上可用".into())
    }
}

/// Build the permission-failure message for the given executable path.
/// Packaged vs. dev produce different guidance because the responsible process
/// differs (the app bundle vs. the terminal that ran `tauri dev`).
#[cfg(target_os = "macos")]
fn permission_error_for(exe: &str) -> String {
    if let Some((bundle_head, _)) = exe.split_once(".app/Contents/") {
        let app_path = format!("{bundle_head}.app");
        format!(
            "辅助功能权限未生效。\n\n\
             灵枢已向系统请求授权（若有弹窗请点「打开系统设置」）。\
             如果列表里「灵枢 LingShu」已勾选却仍出现本提示，是因为 App 更新或重新构建后\
             代码签名发生变化，旧授权已静默失效。请按以下步骤修复：\n\
             1. 打开 系统设置 → 隐私与安全性 → 辅助功能（已自动为你打开）\n\
             2. 选中列表中旧的「灵枢 LingShu」条目，点「−」移除\n\
             3. 点「+」重新添加当前 App：{app_path}\n\
             4. 完全退出灵枢（⌘Q）并重新打开后重试"
        )
    } else {
        format!(
            "辅助功能权限未生效。\n\n\
             当前是开发模式：macOS 会把辅助功能权限归属到启动灵枢的终端 App，\
             此时勾选「灵枢」无效。请按以下步骤修复：\n\
             1. 打开 系统设置 → 隐私与安全性 → 辅助功能（已自动为你打开）\n\
             2. 将运行灵枢的终端 App（Terminal / iTerm2 / VS Code 等）添加到列表并勾选；\
             或点「+」直接添加该可执行文件：{exe}\n\
             3. 重启灵枢后重试"
        )
    }
}

#[cfg(target_os = "macos")]
mod ffi {
    use std::ffi::c_void;

    /// Layout-compatible stand-in for CFDictionary{Key,Value}CallBacks; we only
    /// ever take the address of the exported constants, never read the fields.
    #[repr(C)]
    pub struct CFDictionaryCallBacks {
        pub version: isize,
        pub retain: *const c_void,
        pub release: *const c_void,
        pub copy_description: *const c_void,
        pub equal: *const c_void,
        pub hash: *const c_void,
    }

    extern "C" {
        // HIServices (ApplicationServices) — Accessibility
        pub fn AXIsProcessTrusted() -> bool;
        pub fn AXIsProcessTrustedWithOptions(options: *const c_void) -> bool;
        pub static kAXTrustedCheckOptionPrompt: *const c_void; // CFStringRef
        pub fn AXUIElementCreateApplication(pid: i32) -> *mut c_void;
        pub fn AXUIElementGetTypeID() -> usize;
        pub fn AXUIElementCopyAttributeValue(
            el: *mut c_void,
            attr: *const c_void, // CFStringRef
            val: *mut *mut c_void,
        ) -> i32;
        pub fn AXUIElementSetMessagingTimeout(el: *mut c_void, timeout_secs: f32) -> i32;

        // CoreGraphics — window list (front-to-back z-order)
        pub fn CGWindowListCopyWindowInfo(option: u32, relative_to: u32) -> *mut c_void;

        // CoreFoundation
        pub fn CFRetain(cf: *mut c_void) -> *mut c_void;
        pub fn CFRelease(cf: *mut c_void);
        pub fn CFGetTypeID(cf: *mut c_void) -> usize;
        pub fn CFStringGetTypeID() -> usize;
        pub fn CFArrayGetTypeID() -> usize;
        pub fn CFStringGetLength(s: *mut c_void) -> isize;
        pub fn CFStringGetCString(s: *mut c_void, buf: *mut u8, size: isize, encoding: u32) -> bool;
        pub fn CFStringCreateWithCString(
            alloc: *const c_void,
            c_str: *const std::os::raw::c_char,
            encoding: u32,
        ) -> *mut c_void;
        pub fn CFArrayGetCount(arr: *mut c_void) -> isize;
        pub fn CFArrayGetValueAtIndex(arr: *mut c_void, idx: isize) -> *mut c_void;
        pub fn CFDictionaryGetValue(dict: *mut c_void, key: *const c_void) -> *mut c_void;
        pub fn CFNumberGetValue(num: *mut c_void, number_type: isize, out: *mut c_void) -> bool;
        pub fn CFDictionaryCreate(
            alloc: *const c_void,
            keys: *const *const c_void,
            values: *const *const c_void,
            num_values: isize,
            key_callbacks: *const CFDictionaryCallBacks,
            value_callbacks: *const CFDictionaryCallBacks,
        ) -> *mut c_void;
        pub static kCFTypeDictionaryKeyCallBacks: CFDictionaryCallBacks;
        pub static kCFTypeDictionaryValueCallBacks: CFDictionaryCallBacks;
        pub static kCFBooleanTrue: *const c_void;
    }

    pub const UTF8: u32 = 0x0800_0100; // kCFStringEncodingUTF8
    pub const CF_NUMBER_SINT32: isize = 3; // kCFNumberSInt32Type
    pub const CG_ON_SCREEN_ONLY: u32 = 1 << 0; // kCGWindowListOptionOnScreenOnly
    pub const CG_EXCLUDE_DESKTOP: u32 = 1 << 4; // kCGWindowListExcludeDesktopElements
}

/// Bundle id of our own app — its windows (main + pet) are skipped when
/// choosing what to read.
#[cfg(target_os = "macos")]
const OWN_BUNDLE_ID: &str = "com.lingshu.desktop";

/// System UI / background processes that own on-screen layer-0 windows but
/// hold no user document text (Stage Manager, Dock, Control Center, wallpaper,
/// the compositor, …). Without this denylist the picker can land on
/// `com.apple.WindowManager` and "read" nothing but its own name.
#[cfg(target_os = "macos")]
const SKIP_BUNDLES: &[&str] = &[
    OWN_BUNDLE_ID,
    "com.apple.WindowManager",
    "com.apple.dock",
    "com.apple.controlcenter",
    "com.apple.notificationcenterui",
    "com.apple.systemuiserver",
    "com.apple.wallpaper",
    "com.apple.WindowServer",
    "com.apple.Spotlight",
    "com.apple.coreservices.uiagent",
];

/// Whether a window's owning app should be skipped when picking what to read.
/// `None` (no bundle id) is treated as a system/background window and skipped.
#[cfg(target_os = "macos")]
fn should_skip_bundle(bundle: Option<&str>) -> bool {
    match bundle {
        None => true,
        Some(b) => SKIP_BUNDLES.contains(&b),
    }
}

/// Opaque CoreFoundation object (CFTypeRef / AXUIElementRef / …).
#[cfg(target_os = "macos")]
type CFRef = std::ffi::c_void;

#[cfg(target_os = "macos")]
unsafe fn read_frontmost() -> Result<String, String> {
    if !ax_trusted_with_prompt() {
        open_accessibility_settings_once();
        let exe = std::env::current_exe()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "<unknown>".into());
        return Err(permission_error_for(&exe));
    }
    let (pid, app_name) =
        pick_target_app().ok_or_else(|| "无法确定要读取的前台应用".to_string())?;
    read_app_text(pid, &app_name)
}

/// Check AX trust; when missing, ask with the system prompt. The prompt is
/// attributed to this process (`com.lingshu.desktop`) and registers that entry
/// with the *current* code signature — the only reliable way to avoid stale
/// entries after a rebuild.
#[cfg(target_os = "macos")]
unsafe fn ax_trusted_with_prompt() -> bool {
    use std::ptr;
    if ffi::AXIsProcessTrusted() {
        return true;
    }
    let keys = [ffi::kAXTrustedCheckOptionPrompt];
    let values = [ffi::kCFBooleanTrue];
    let options = ffi::CFDictionaryCreate(
        ptr::null(),
        keys.as_ptr(),
        values.as_ptr(),
        1,
        &ffi::kCFTypeDictionaryKeyCallBacks,
        &ffi::kCFTypeDictionaryValueCallBacks,
    );
    let trusted = ffi::AXIsProcessTrustedWithOptions(options);
    if !options.is_null() {
        ffi::CFRelease(options);
    }
    trusted
}

/// Open the Accessibility pane of System Settings, at most once per process,
/// so repeated failed reads don't keep stealing focus.
#[cfg(target_os = "macos")]
fn open_accessibility_settings_once() {
    use std::sync::atomic::{AtomicBool, Ordering};
    static OPENED: AtomicBool = AtomicBool::new(false);
    if !OPENED.swap(true, Ordering::SeqCst) {
        let _ = std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
            .spawn();
    }
}

/// Create a CFString from a Rust string. Caller must CFRelease.
#[cfg(target_os = "macos")]
unsafe fn cfstr(s: &str) -> *mut CFRef {
    let c = std::ffi::CString::new(s).expect("no NUL in attribute names");
    ffi::CFStringCreateWithCString(std::ptr::null(), c.as_ptr(), ffi::UTF8)
}

/// Convert a CFTypeRef to String iff it actually is a CFString (AXValue can
/// also be a CFNumber/CFBoolean for sliders, checkboxes, …). Does not consume.
#[cfg(target_os = "macos")]
unsafe fn cfstring_to_string(s: *mut CFRef) -> Option<String> {
    if s.is_null() || ffi::CFGetTypeID(s) != ffi::CFStringGetTypeID() {
        return None;
    }
    let len = ffi::CFStringGetLength(s);
    let max = len * 4 + 1;
    let mut buf = vec![0u8; max as usize];
    if !ffi::CFStringGetCString(s, buf.as_mut_ptr(), max, ffi::UTF8) {
        return None;
    }
    let end = buf.iter().position(|&b| b == 0).unwrap_or(0);
    String::from_utf8(buf[..end].to_vec()).ok()
}

/// Copy an AX attribute (caller must CFRelease the returned value).
#[cfg(target_os = "macos")]
unsafe fn ax_copy(el: *mut CFRef, attr: &str) -> Option<*mut CFRef> {
    // Only query genuine AXUIElements. AX child arrays can occasionally surface
    // non-element values, and AXUIElementCopyAttributeValue *traps* (rather than
    // erroring) when handed a non-element — that was the SIGTRAP in _AXUIElementValidate.
    if el.is_null() || ffi::CFGetTypeID(el) != ffi::AXUIElementGetTypeID() {
        return None;
    }
    let name = cfstr(attr);
    if name.is_null() {
        return None;
    }
    let mut val: *mut CFRef = std::ptr::null_mut();
    let err = ffi::AXUIElementCopyAttributeValue(el, name, &mut val);
    ffi::CFRelease(name);
    if err != 0 || val.is_null() {
        None
    } else {
        Some(val)
    }
}

/// Copy a string-valued AX attribute.
#[cfg(target_os = "macos")]
unsafe fn ax_string(el: *mut CFRef, attr: &str) -> Option<String> {
    let val = ax_copy(el, attr)?;
    let s = cfstring_to_string(val);
    ffi::CFRelease(val);
    s.filter(|s| !s.trim().is_empty())
}

/// Read an i32 entry out of a CFDictionary (CG window info).
#[cfg(target_os = "macos")]
unsafe fn dict_i32(dict: *mut CFRef, key: *mut CFRef) -> Option<i32> {
    let num = ffi::CFDictionaryGetValue(dict, key);
    if num.is_null() {
        return None;
    }
    let mut out: i32 = 0;
    if ffi::CFNumberGetValue(num, ffi::CF_NUMBER_SINT32, &mut out as *mut i32 as *mut _) {
        Some(out)
    } else {
        None
    }
}

/// (bundle id, localized name) of the app owning `pid`, via NSRunningApplication.
#[cfg(target_os = "macos")]
unsafe fn app_info(pid: i32) -> (Option<String>, Option<String>) {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    use std::ffi::CStr;

    unsafe fn ns_string(obj: *mut AnyObject) -> Option<String> {
        use objc2::msg_send;
        if obj.is_null() {
            return None;
        }
        let p: *const std::os::raw::c_char = msg_send![obj, UTF8String];
        if p.is_null() {
            return None;
        }
        Some(CStr::from_ptr(p).to_string_lossy().into_owned())
    }

    let cls = objc2::class!(NSRunningApplication);
    let app: *mut AnyObject = msg_send![cls, runningApplicationWithProcessIdentifier: pid];
    if app.is_null() {
        return (None, None);
    }
    let bundle: *mut AnyObject = msg_send![app, bundleIdentifier];
    let name: *mut AnyObject = msg_send![app, localizedName];
    (ns_string(bundle), ns_string(name))
}

/// Pick the app to read: the frontmost on-screen, layer-0 window that does NOT
/// belong to LingShu itself — talking to the assistant usually puts our own
/// window in front, and reading our chat back would be useless. Falls back to
/// NSWorkspace's frontmost application.
#[cfg(target_os = "macos")]
unsafe fn pick_target_app() -> Option<(i32, String)> {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;

    let own_pid = std::process::id() as i32;

    let list =
        ffi::CGWindowListCopyWindowInfo(ffi::CG_ON_SCREEN_ONLY | ffi::CG_EXCLUDE_DESKTOP, 0);
    if !list.is_null() {
        let key_pid = cfstr("kCGWindowOwnerPID");
        let key_layer = cfstr("kCGWindowLayer");
        let mut found: Option<(i32, String)> = None;
        for i in 0..ffi::CFArrayGetCount(list) {
            let win = ffi::CFArrayGetValueAtIndex(list, i);
            if win.is_null() {
                continue;
            }
            // Layer 0 = normal windows; skips menu bar, Dock and floating
            // panels (including the always-on-top pet window).
            if dict_i32(win, key_layer) != Some(0) {
                continue;
            }
            let Some(pid) = dict_i32(win, key_pid) else {
                continue;
            };
            if pid == own_pid {
                continue;
            }
            let (bundle, name) = app_info(pid);
            if should_skip_bundle(bundle.as_deref()) {
                continue;
            }
            found = Some((pid, name.unwrap_or_else(|| format!("pid {pid}"))));
            break;
        }
        ffi::CFRelease(key_pid);
        ffi::CFRelease(key_layer);
        ffi::CFRelease(list);
        if found.is_some() {
            return found;
        }
    }

    // Fallback: NSWorkspace frontmost application.
    let workspace_cls = objc2::class!(NSWorkspace);
    let workspace: *mut AnyObject = msg_send![workspace_cls, sharedWorkspace];
    let front: *mut AnyObject = msg_send![workspace, frontmostApplication];
    if front.is_null() {
        return None;
    }
    let pid: i32 = msg_send![front, processIdentifier];
    let (bundle, name) = app_info(pid);
    if should_skip_bundle(bundle.as_deref()) {
        return None;
    }
    Some((pid, name.unwrap_or_else(|| format!("pid {pid}"))))
}

/// Per-read extraction limits. AX calls are IPC round-trips, so the walk is
/// bounded in breadth and total output size.
#[cfg(target_os = "macos")]
const MAX_DEPTH: usize = 30;
#[cfg(target_os = "macos")]
const MAX_VISITED: usize = 3000;
#[cfg(target_os = "macos")]
const MAX_TOTAL_CHARS: usize = 16000;
#[cfg(target_os = "macos")]
const MAX_ELEMENT_CHARS: usize = 3000;

#[cfg(target_os = "macos")]
struct WalkState {
    visited: usize,
    chars: usize,
    seen: std::collections::HashSet<String>,
    out: Vec<String>,
}

/// Depth-first walk over the AX tree collecting visible text (roughly in
/// reading order). Dedupes repeated strings and stops at the budgets above.
#[cfg(target_os = "macos")]
unsafe fn collect_text(el: *mut CFRef, depth: usize, st: &mut WalkState) {
    if depth > MAX_DEPTH || st.visited >= MAX_VISITED || st.chars >= MAX_TOTAL_CHARS {
        return;
    }
    st.visited += 1;

    if let Some(role) = ax_string(el, "AXRole") {
        if matches!(
            role.as_str(),
            "AXScrollBar" | "AXSplitter" | "AXGrowArea" | "AXMenuBar" | "AXMenuBarItem"
        ) {
            return;
        }
    }

    // Text payload: try multiple attributes that different frameworks use.
    // AXValue covers text fields, sliders, static text.
    // AXTitle covers buttons, tabs, labels.
    // AXDescription covers images, web elements, and Electron app content.
    // AXRoleDescription provides human-readable role info for some elements.
    // AXSelectedText is handled separately in read_app_text for the focused element.
    let text = ax_string(el, "AXValue")
        .or_else(|| ax_string(el, "AXTitle"))
        .or_else(|| ax_string(el, "AXDescription"))
        .or_else(|| ax_string(el, "AXRoleDescription"));
    if let Some(t) = text {
        let t = t.trim();
        let t: String = if t.chars().count() > MAX_ELEMENT_CHARS {
            let mut s: String = t.chars().take(MAX_ELEMENT_CHARS).collect();
            s.push('…');
            s
        } else {
            t.to_string()
        };
        if !t.is_empty() && st.seen.insert(t.clone()) {
            st.chars += t.chars().count();
            st.out.push(t);
        }
    }

    if let Some(children) = ax_copy(el, "AXChildren") {
        if ffi::CFGetTypeID(children) == ffi::CFArrayGetTypeID() {
            let n = ffi::CFArrayGetCount(children);
            for i in 0..n {
                let child = ffi::CFArrayGetValueAtIndex(children, i);
                // Borrowed (Get rule). Skip anything that isn't an AXUIElement,
                // and retain across the recursive walk so AX run-loop pumping
                // can't free it under us.
                if !child.is_null() && ffi::CFGetTypeID(child) == ffi::AXUIElementGetTypeID() {
                    ffi::CFRetain(child);
                    collect_text(child, depth + 1, st);
                    ffi::CFRelease(child);
                }
            }
        }
        ffi::CFRelease(children);
    }
}

/// Depth-limited BFS to find the shallowest AXWebArea inside an AXWindow, so we
/// can start text collection from the actual page content instead of walking
/// through the entire browser chrome. Returns an **owned (+1)** reference the
/// caller must `CFRelease`, or `None`.
///
/// Ownership invariant (each owned ref is released exactly once):
///   - `window` is borrowed — the caller owns it; this fn never releases it.
///   - children popped from AX arrays are retained on push and released once,
///     either after we finish expanding them or, on early return, by the final
///     drain of whatever is still queued.
///   - the returned web area is owned: a retained child handed off as-is, or a
///     freshly retained `window` when the window itself is the web area.
///
/// The previous version released over-deep elements in the loop *and* again in
/// the final drain (they were never removed from the queue) — a double free
/// that crashed `CFRelease` with a pointer-authentication trap on deep, non-web
/// UI trees. Popping each element off the queue before handling it makes a
/// second release impossible.
#[cfg(target_os = "macos")]
unsafe fn find_first_webarea(window: *mut CFRef) -> Option<*mut CFRef> {
    // (element, depth, owned). `owned` distinguishes the borrowed initial
    // window from retained children so we release each exactly once.
    let mut queue: std::collections::VecDeque<(*mut CFRef, usize, bool)> =
        std::collections::VecDeque::new();
    queue.push_back((window, 0, false));

    let mut result = None;
    while let Some((el, depth, owned)) = queue.pop_front() {
        if el.is_null() {
            continue;
        }
        if ax_string(el, "AXRole").as_deref() == Some("AXWebArea") {
            // Hand off ownership: retain the borrowed window if it is itself
            // the web area; an owned child is returned as-is. Remaining queued
            // children are released by the drain below.
            result = Some(if owned { el } else { ffi::CFRetain(el) });
            break;
        }
        if depth <= 8 {
            if let Some(children) = ax_copy(el, "AXChildren") {
                if ffi::CFGetTypeID(children) == ffi::CFArrayGetTypeID() {
                    let n = ffi::CFArrayGetCount(children);
                    for i in 0..n {
                        let child = ffi::CFArrayGetValueAtIndex(children, i);
                        if !child.is_null() {
                            ffi::CFRetain(child);
                            queue.push_back((child, depth + 1, true));
                        }
                    }
                }
                ffi::CFRelease(children);
            }
        }
        // Done with this element — release it iff we own it (never the
        // borrowed window, never the returned web area which broke above).
        if owned {
            ffi::CFRelease(el);
        }
    }

    // Release any still-owned elements left unprocessed by an early break.
    for (el, _depth, owned) in queue.drain(..) {
        if owned && !el.is_null() {
            ffi::CFRelease(el);
        }
    }
    result
}

#[cfg(target_os = "macos")]
unsafe fn read_app_text(pid: i32, app_name: &str) -> Result<String, String> {
    let ax_app = ffi::AXUIElementCreateApplication(pid);
    if ax_app.is_null() {
        return Err(format!("无法为 {app_name} 创建辅助功能元素"));
    }
    // Don't hang the read on an unresponsive app.
    ffi::AXUIElementSetMessagingTimeout(ax_app, 1.5);

    let mut lines = vec![format!("[前台应用] {app_name}")];

    let window = ax_copy(ax_app, "AXFocusedWindow")
        .or_else(|| ax_copy(ax_app, "AXMainWindow"))
        .or_else(|| {
            let wins = ax_copy(ax_app, "AXWindows")?;
            let mut first = None;
            if ffi::CFGetTypeID(wins) == ffi::CFArrayGetTypeID() && ffi::CFArrayGetCount(wins) > 0
            {
                let w = ffi::CFArrayGetValueAtIndex(wins, 0);
                if !w.is_null() {
                    // Items are borrowed from the array (Get rule) — retain to
                    // outlive the array's release below.
                    first = Some(ffi::CFRetain(w));
                }
            }
            ffi::CFRelease(wins);
            first
        });

    if let Some(win) = window {
        if let Some(title) = ax_string(win, "AXTitle") {
            lines.push(format!("[窗口] {title}"));
        }
    }

    // Selected text is the highest-signal content — surface it first.
    if let Some(focused) = ax_copy(ax_app, "AXFocusedUIElement") {
        if let Some(sel) = ax_string(focused, "AXSelectedText") {
            lines.push(format!("[选中文本] {sel}"));
        }
        ffi::CFRelease(focused);
    }

    // Walk the focused window. For browsers and other complex apps, prefer
    // starting from the AXWebArea (the actual page content) rather than the
    // window root, which is full of toolbar/tab chrome noise. The web area, if
    // found, is an owned (+1) reference distinct from the window — released
    // below alongside the window and app element (each exactly once).
    let webarea = window.and_then(|win| find_first_webarea(win));
    let start = webarea.or(window).unwrap_or(ax_app);

    let mut st = WalkState {
        visited: 0,
        chars: 0,
        seen: std::collections::HashSet::new(),
        out: Vec::new(),
    };
    collect_text(start, 0, &mut st);
    if !st.out.is_empty() {
        lines.push("[内容]".into());
        lines.extend(st.out);
    }

    if let Some(wa) = webarea {
        ffi::CFRelease(wa);
    }
    if let Some(win) = window {
        ffi::CFRelease(win);
    }
    ffi::CFRelease(ax_app);

    if lines.len() <= 1 {
        lines.push(
            "(该应用未暴露可读的辅助功能文本——可能是全图形界面，或需要在该应用内开启辅助功能支持)"
                .into(),
        );
    }
    Ok(lines.join("\n"))
}

/// Request Accessibility permission. Triggers the macOS system prompt, which
/// registers 「灵枢 LingShu」 (`com.lingshu.desktop`) in the Accessibility list
/// with the *current* code signature. This is what fixes the "已勾选却仍未授权"
/// state: a rebuilt app's old entry no longer matches its signature, and only
/// the system prompt re-registers it correctly. Also opens the settings pane so
/// the user can flip the switch (or remove a stale entry) right away.
/// `prompt = false` is a silent status check (used at startup); the default
/// `true` fires the system dialog + opens System Settings (used by the
/// one-click grant button at point of need).
#[tauri::command]
pub fn request_accessibility_permission(prompt: Option<bool>) -> Result<bool, String> {
    let want_prompt = prompt.unwrap_or(true);
    #[cfg(target_os = "macos")]
    {
        let trusted = unsafe {
            if ffi::AXIsProcessTrusted() {
                return Ok(true);
            }
            if !want_prompt {
                return Ok(false);
            }
            ax_trusted_with_prompt()
        };
        if trusted {
            return Ok(true);
        }
        open_accessibility_settings_once();
        Ok(false)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = want_prompt;
        Err("仅 macOS 支持".into())
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[test]
    fn permission_error_detects_bundled_app() {
        let msg =
            permission_error_for("/Applications/灵枢 LingShu.app/Contents/MacOS/lingshu-tauri");
        assert!(msg.contains("/Applications/灵枢 LingShu.app"));
        assert!(msg.contains("移除"));
    }

    #[test]
    fn permission_error_detects_dev_binary() {
        let msg = permission_error_for("/Users/me/projects/PA/src-tauri/target/debug/lingshu-tauri");
        assert!(msg.contains("终端"));
        assert!(msg.contains("target/debug/lingshu-tauri"));
    }

    /// Exercises the CFString FFI both ways — signature mistakes here crash at
    /// runtime, not compile time.
    #[test]
    fn cfstring_roundtrip() {
        unsafe {
            let s = cfstr("灵枢 LingShu — ÅB€ test");
            assert!(!s.is_null());
            let back = cfstring_to_string(s);
            ffi::CFRelease(s);
            assert_eq!(back.as_deref(), Some("灵枢 LingShu — ÅB€ test"));
        }
    }
}
