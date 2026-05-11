//! 划词语音问答（QA）专用的全局快捷键监听器。
//!
//! 与 `hotkey.rs`（modifier-only 听写热键）平行——QA 用的是组合键
//! `Cmd+Shift+;` / `Ctrl+Shift+;`，所以走 `global-hotkey` crate（macOS 内部
//! 用 Carbon `RegisterEventHotKey`，Windows 用 `RegisterHotKey`，Linux 用 X11）。
//!
//! 仅产出 `QaHotkeyEvent::Pressed` 边沿事件；toggle / 录音生命周期由
//! coordinator 解释（第一次按 → 开始问答；第二次按 → 结束）。
//!
//! 模块依赖：仅 `types`，与 CLAUDE.md "Rust 模块依赖只通过 types.rs 跨模块" 一致。

use std::sync::mpsc::Sender;
use std::sync::Arc;

use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use parking_lot::Mutex;

use crate::types::QaHotkeyBinding;

#[derive(Debug, Clone, Copy)]
pub enum QaHotkeyEvent {
    /// 用户按下了配置的 QA 组合键（toggle 模式：第一次开始，第二次结束）。
    Pressed,
}

#[derive(Debug, thiserror::Error)]
pub enum QaHotkeyError {
    #[error("不支持的修饰键: {0}")]
    UnsupportedModifier(String),
    #[error("不支持的主键: {0}")]
    UnsupportedKey(String),
    #[error("注册全局快捷键失败: {0}")]
    RegisterFailed(String),
    #[error("初始化全局快捷键管理器失败: {0}")]
    ManagerInitFailed(String),
}

/// QA 全局快捷键监听器。`Drop` 时反注册。
///
/// 内部用 `global-hotkey` crate；事件转发线程持有一个共享的 `Sender`。
pub struct QaHotkeyMonitor {
    inner: Arc<Inner>,
}

struct Inner {
    manager: GlobalHotKeyManager,
    /// 当前注册的 hotkey 句柄；用于 unregister。
    registered: Mutex<Option<HotKey>>,
    /// 事件转发线程接收 global-hotkey crate 的全局 channel，再过滤 id 后转发到 tx。
    forward_alive: Arc<std::sync::atomic::AtomicBool>,
    /// 当前关心的 hotkey id（filter 用）。
    active_id: Arc<std::sync::atomic::AtomicU32>,
}

// global-hotkey 0.6 的 GlobalHotKeyManager 在 Windows 内部持有 HHOOK / window
// handle 等 `*mut c_void`，crate 没标 Send/Sync。但这些句柄实际是 OS 进程级
// 资源，跨线程读写是 OS 自己同步的；coordinator.rs 又需要把 `Arc<Inner>`（间接含
// QaHotkeyMonitor）放进 async_runtime::spawn 里，强制要求 Send。手动标记。
// macOS 上 GlobalHotKeyManager 内部用 Carbon EventHotKey，同理。
// 与 hotkey.rs::CallbackContext 已有的 unsafe impl Send/Sync 同款做法。
unsafe impl Send for Inner {}
unsafe impl Sync for Inner {}

impl QaHotkeyMonitor {
    /// 启动监听并注册一个 hotkey。`tx` 在每次按下边沿收到 `QaHotkeyEvent::Pressed`。
    ///
    /// **注意**：`global-hotkey` crate 在 macOS 要求 manager 在主线程构造。
    /// 调用方需要确保从主线程触发（coordinator 的 supervisor 线程会通过
    /// `AppHandle::run_on_main_thread` 跳到主线程后再 spawn 这个 monitor）。
    /// 本函数不强制断言主线程——单元 / 集成测试也跑不到 manager 创建那一行。
    pub fn start(
        binding: QaHotkeyBinding,
        tx: Sender<QaHotkeyEvent>,
    ) -> Result<Self, QaHotkeyError> {
        let manager = GlobalHotKeyManager::new()
            .map_err(|e| QaHotkeyError::ManagerInitFailed(e.to_string()))?;

        let hotkey = parse_binding(&binding)?;
        manager
            .register(hotkey)
            .map_err(|e| QaHotkeyError::RegisterFailed(e.to_string()))?;

        let active_id = Arc::new(std::sync::atomic::AtomicU32::new(hotkey.id()));
        let forward_alive = Arc::new(std::sync::atomic::AtomicBool::new(true));

        // 启动转发线程：消费 global-hotkey 的进程级 channel，filter id 后投递到上层 tx。
        // global-hotkey 用 crossbeam_channel，自带超时 recv，便于优雅退出。
        let alive_for_thread = Arc::clone(&forward_alive);
        let id_for_thread = Arc::clone(&active_id);
        std::thread::Builder::new()
            .name("openless-qa-hotkey-forward".into())
            .spawn(move || forward_loop(alive_for_thread, id_for_thread, tx))
            .map_err(|e| QaHotkeyError::RegisterFailed(format!("spawn forward thread: {e}")))?;

        Ok(Self {
            inner: Arc::new(Inner {
                manager,
                registered: Mutex::new(Some(hotkey)),
                forward_alive,
                active_id,
            }),
        })
    }

    /// 替换当前注册的 hotkey（用户在设置里改了组合键时）。
    pub fn update_binding(&self, binding: QaHotkeyBinding) -> Result<(), QaHotkeyError> {
        let next = parse_binding(&binding)?;
        let mut current = self.inner.registered.lock();
        if let Some(prev) = current.take() {
            if prev == next {
                *current = Some(prev);
                return Ok(());
            }
            if let Err(e) = self.inner.manager.unregister(prev) {
                log::warn!("[qa-hotkey] unregister 旧绑定失败: {e}");
            }
        }
        self.inner
            .manager
            .register(next)
            .map_err(|e| QaHotkeyError::RegisterFailed(e.to_string()))?;
        *current = Some(next);
        self.inner
            .active_id
            .store(next.id(), std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
}

impl Drop for QaHotkeyMonitor {
    fn drop(&mut self) {
        // 通知转发线程退出；超时 recv 后自然结束。
        self.inner
            .forward_alive
            .store(false, std::sync::atomic::Ordering::SeqCst);
        if let Some(prev) = self.inner.registered.lock().take() {
            if let Err(e) = self.inner.manager.unregister(prev) {
                log::warn!("[qa-hotkey] drop 时 unregister 失败: {e}");
            }
        }
    }
}

fn forward_loop(
    alive: Arc<std::sync::atomic::AtomicBool>,
    active_id: Arc<std::sync::atomic::AtomicU32>,
    tx: Sender<QaHotkeyEvent>,
) {
    // global-hotkey crate 用 crossbeam_channel；其 receiver 没暴露 RecvTimeoutError 给外部，
    // 所以不区分 timeout vs disconnect，统一 250ms tick 重新 check alive 标志。
    let receiver = GlobalHotKeyEvent::receiver();
    while alive.load(std::sync::atomic::Ordering::SeqCst) {
        let event = match receiver.recv_timeout(std::time::Duration::from_millis(250)) {
            Ok(e) => e,
            Err(_) => continue,
        };
        let want = active_id.load(std::sync::atomic::Ordering::SeqCst);
        if event.id() != want {
            continue;
        }
        if !matches!(event.state(), HotKeyState::Pressed) {
            continue;
        }
        if let Err(e) = tx.send(QaHotkeyEvent::Pressed) {
            log::warn!("[qa-hotkey] 事件投递失败: {e}");
            break;
        }
    }
    log::info!("[qa-hotkey] 转发线程退出");
}

fn parse_binding(binding: &QaHotkeyBinding) -> Result<HotKey, QaHotkeyError> {
    let mut mods = Modifiers::empty();
    for raw in &binding.modifiers {
        let tag = normalize_modifier_tag(raw);
        let bit = match tag.as_str() {
            "cmd" | "command" | "super" | "meta" | "win" => Modifiers::SUPER,
            "ctrl" | "control" => Modifiers::CONTROL,
            "alt" | "option" | "opt" => Modifiers::ALT,
            "shift" => Modifiers::SHIFT,
            other => return Err(QaHotkeyError::UnsupportedModifier(other.to_string())),
        };
        mods |= bit;
    }
    let code = parse_primary(&binding.primary)?;
    Ok(HotKey::new(Some(mods), code))
}

fn normalize_modifier_tag(raw: &str) -> String {
    let tag = raw.trim().to_ascii_lowercase();
    #[cfg(target_os = "windows")]
    {
        if matches!(tag.as_str(), "cmd" | "command") {
            return "ctrl".to_string();
        }
    }
    tag
}

/// 把用户配置的主键字符串解析成 keyboard_types::Code。
/// 支持单字符（字母 / 数字 / 符号）+ 常见命名键（F1..F12 / Enter / Tab / Escape / Space）。
fn parse_primary(raw: &str) -> Result<Code, QaHotkeyError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(QaHotkeyError::UnsupportedKey("(空)".into()));
    }
    // 单字符
    if trimmed.chars().count() == 1 {
        let ch = trimmed.chars().next().unwrap();
        if let Some(code) = char_to_code(ch) {
            return Ok(code);
        }
    }
    // 命名键
    let upper = trimmed.to_ascii_uppercase();
    let named = match upper.as_str() {
        "ENTER" | "RETURN" => Code::Enter,
        "TAB" => Code::Tab,
        "ESC" | "ESCAPE" => Code::Escape,
        "SPACE" => Code::Space,
        "BACKSPACE" => Code::Backspace,
        "DELETE" | "DEL" => Code::Delete,
        "HOME" => Code::Home,
        "END" => Code::End,
        "PAGEUP" => Code::PageUp,
        "PAGEDOWN" => Code::PageDown,
        "ARROWUP" | "UP" => Code::ArrowUp,
        "ARROWDOWN" | "DOWN" => Code::ArrowDown,
        "ARROWLEFT" | "LEFT" => Code::ArrowLeft,
        "ARROWRIGHT" | "RIGHT" => Code::ArrowRight,
        "F1" => Code::F1,
        "F2" => Code::F2,
        "F3" => Code::F3,
        "F4" => Code::F4,
        "F5" => Code::F5,
        "F6" => Code::F6,
        "F7" => Code::F7,
        "F8" => Code::F8,
        "F9" => Code::F9,
        "F10" => Code::F10,
        "F11" => Code::F11,
        "F12" => Code::F12,
        _ => return Err(QaHotkeyError::UnsupportedKey(trimmed.to_string())),
    };
    Ok(named)
}

fn char_to_code(ch: char) -> Option<Code> {
    let c = ch.to_ascii_uppercase();
    let code = match c {
        'A' => Code::KeyA,
        'B' => Code::KeyB,
        'C' => Code::KeyC,
        'D' => Code::KeyD,
        'E' => Code::KeyE,
        'F' => Code::KeyF,
        'G' => Code::KeyG,
        'H' => Code::KeyH,
        'I' => Code::KeyI,
        'J' => Code::KeyJ,
        'K' => Code::KeyK,
        'L' => Code::KeyL,
        'M' => Code::KeyM,
        'N' => Code::KeyN,
        'O' => Code::KeyO,
        'P' => Code::KeyP,
        'Q' => Code::KeyQ,
        'R' => Code::KeyR,
        'S' => Code::KeyS,
        'T' => Code::KeyT,
        'U' => Code::KeyU,
        'V' => Code::KeyV,
        'W' => Code::KeyW,
        'X' => Code::KeyX,
        'Y' => Code::KeyY,
        'Z' => Code::KeyZ,
        '0' => Code::Digit0,
        '1' => Code::Digit1,
        '2' => Code::Digit2,
        '3' => Code::Digit3,
        '4' => Code::Digit4,
        '5' => Code::Digit5,
        '6' => Code::Digit6,
        '7' => Code::Digit7,
        '8' => Code::Digit8,
        '9' => Code::Digit9,
        ';' => Code::Semicolon,
        ':' => Code::Semicolon,
        ',' => Code::Comma,
        '.' => Code::Period,
        '/' => Code::Slash,
        '\\' => Code::Backslash,
        '[' => Code::BracketLeft,
        ']' => Code::BracketRight,
        '\'' => Code::Quote,
        '`' => Code::Backquote,
        '-' => Code::Minus,
        '=' => Code::Equal,
        ' ' => Code::Space,
        _ => return None,
    };
    Some(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_default_binding() {
        let binding = QaHotkeyBinding::default();
        let parsed = parse_binding(&binding).expect("default binding parses");
        assert!(parsed.mods.contains(Modifiers::SHIFT));
        assert_eq!(parsed.key, Code::Semicolon);
    }

    #[test]
    fn parse_letter_binding() {
        let binding = QaHotkeyBinding {
            primary: "k".into(),
            modifiers: vec!["cmd".into(), "alt".into()],
        };
        let parsed = parse_binding(&binding).expect("letter binding parses");
        assert_eq!(parsed.key, Code::KeyK);
        assert!(parsed.mods.contains(Modifiers::SUPER));
        assert!(parsed.mods.contains(Modifiers::ALT));
    }

    #[test]
    fn unsupported_modifier_rejected() {
        let binding = QaHotkeyBinding {
            primary: ";".into(),
            modifiers: vec!["hyper".into()],
        };
        assert!(matches!(
            parse_binding(&binding),
            Err(QaHotkeyError::UnsupportedModifier(_))
        ));
    }

    #[test]
    fn empty_primary_rejected() {
        let binding = QaHotkeyBinding {
            primary: "".into(),
            modifiers: vec!["cmd".into()],
        };
        assert!(matches!(
            parse_binding(&binding),
            Err(QaHotkeyError::UnsupportedKey(_))
        ));
    }

    #[test]
    fn cmd_modifier_normalizes_per_platform() {
        let binding = QaHotkeyBinding {
            primary: ";".into(),
            modifiers: vec!["cmd".into(), "shift".into()],
        };
        let parsed = parse_binding(&binding).expect("binding parses");

        #[cfg(target_os = "windows")]
        {
            assert!(parsed.mods.contains(Modifiers::CONTROL));
            assert!(!parsed.mods.contains(Modifiers::SUPER));
        }

        #[cfg(not(target_os = "windows"))]
        {
            assert!(parsed.mods.contains(Modifiers::SUPER));
        }
    }
}
