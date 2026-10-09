//! Windows WH_KEYBOARD_LL hook (never blocks keys). Owned by Task 4.
//! The key tracking is plain Rust so it is unit-tested on every platform; only `imp` is Windows.

use super::{KeyState, Modifier};
use std::collections::BTreeSet;

const VK_TAB: u32 = 0x09;
const VK_SHIFT: u32 = 0x10;
const VK_CONTROL: u32 = 0x11;
const VK_MENU: u32 = 0x12;
const VK_ESCAPE: u32 = 0x1B;
const VK_SPACE: u32 = 0x20;
const VK_DELETE: u32 = 0x2E;
const VK_LWIN: u32 = 0x5B;
const VK_RWIN: u32 = 0x5C;
const VK_F1: u32 = 0x70;
const VK_F24: u32 = 0x87;
const VK_LSHIFT: u32 = 0xA0;
const VK_RSHIFT: u32 = 0xA1;
const VK_LCONTROL: u32 = 0xA2;
const VK_RCONTROL: u32 = 0xA3;
const VK_LMENU: u32 = 0xA4;
const VK_RMENU: u32 = 0xA5;

fn modifier_of(vk: u32) -> Option<Modifier> {
    match vk {
        VK_SHIFT | VK_LSHIFT | VK_RSHIFT => Some(Modifier::Shift),
        VK_CONTROL | VK_LCONTROL | VK_RCONTROL => Some(Modifier::Ctrl),
        VK_MENU | VK_LMENU | VK_RMENU => Some(Modifier::Alt),
        VK_LWIN | VK_RWIN => Some(Modifier::Meta),
        _ => None,
    }
}

// Names from the combo alphabet; Tab and Delete are named only so validation can refuse them.
fn key_name(vk: u32) -> String {
    match vk {
        0x30..=0x39 | 0x41..=0x5A => char::from_u32(vk).map_or_else(String::new, String::from),
        VK_F1..=VK_F24 => format!("F{}", vk - VK_F1 + 1),
        VK_SPACE => "Space".into(),
        VK_TAB => "Tab".into(),
        VK_DELETE => "Delete".into(),
        _ => "Other".into(),
    }
}

// Tracks only the keys currently down, in press order.
#[derive(Default)]
struct WinKeys {
    down: Vec<u32>,
}

impl WinKeys {
    fn update(&mut self, vk: u32, is_down: bool) -> KeyState {
        if !is_down {
            self.down.retain(|k| *k != vk);
        } else if !self.down.contains(&vk) {
            self.down.push(vk);
        }
        let modifiers: BTreeSet<Modifier> =
            self.down.iter().filter_map(|k| modifier_of(*k)).collect();
        let escape = self.down.contains(&VK_ESCAPE);
        let key = self
            .down
            .iter()
            .rev()
            .find(|k| modifier_of(**k).is_none() && **k != VK_ESCAPE)
            .map(|k| key_name(*k));
        KeyState {
            modifiers,
            key,
            escape,
        }
    }
}

#[cfg(windows)]
pub use imp::{spawn, Guard};

#[cfg(windows)]
mod imp {
    use super::*;
    use crate::hotkey::{dispatch, Core, HotkeyEvent};
    use std::ptr;
    use std::sync::mpsc::{self, Sender};
    use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
    use std::thread::{self, JoinHandle};
    use std::time::Duration;
    use windows_sys::Win32::Foundation::{GetLastError, LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::System::Threading::GetCurrentThreadId;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse as vk;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, GetMessageW, PeekMessageW, PostThreadMessageW, SetWindowsHookExW,
        UnhookWindowsHookEx, HC_ACTION, HHOOK, KBDLLHOOKSTRUCT, MSG, PM_NOREMOVE, WH_KEYBOARD_LL,
        WM_KEYDOWN, WM_KEYUP, WM_QUIT, WM_SYSKEYDOWN, WM_SYSKEYUP,
    };

    // Compile-time proof that the hand-copied codes above match windows-sys.
    const _: () = {
        assert!(VK_TAB == vk::VK_TAB as u32);
        assert!(VK_SHIFT == vk::VK_SHIFT as u32);
        assert!(VK_CONTROL == vk::VK_CONTROL as u32);
        assert!(VK_MENU == vk::VK_MENU as u32);
        assert!(VK_ESCAPE == vk::VK_ESCAPE as u32);
        assert!(VK_SPACE == vk::VK_SPACE as u32);
        assert!(VK_DELETE == vk::VK_DELETE as u32);
        assert!(VK_LWIN == vk::VK_LWIN as u32);
        assert!(VK_RWIN == vk::VK_RWIN as u32);
        assert!(VK_F1 == vk::VK_F1 as u32);
        assert!(VK_F24 == vk::VK_F24 as u32);
        assert!(VK_LSHIFT == vk::VK_LSHIFT as u32);
        assert!(VK_RSHIFT == vk::VK_RSHIFT as u32);
        assert!(VK_LCONTROL == vk::VK_LCONTROL as u32);
        assert!(VK_RCONTROL == vk::VK_RCONTROL as u32);
        assert!(VK_LMENU == vk::VK_LMENU as u32);
        assert!(VK_RMENU == vk::VK_RMENU as u32);
    };

    struct Ctx {
        core: Arc<Mutex<Core>>,
        tx: Sender<HotkeyEvent>,
        keys: WinKeys,
    }

    // The hook callback has no user pointer, so its context lives in a static (one hook at a time).
    static CTX: Mutex<Option<Ctx>> = Mutex::new(None);

    fn lock_ctx() -> MutexGuard<'static, Option<Ctx>> {
        CTX.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn on_key(message: u32, vk_code: u32) {
        let is_down = match message {
            WM_KEYDOWN | WM_SYSKEYDOWN => true,
            WM_KEYUP | WM_SYSKEYUP => false,
            _ => return,
        };
        let mut slot = lock_ctx();
        let Some(ctx) = slot.as_mut() else { return };
        let state = ctx.keys.update(vk_code, is_down);
        dispatch(&ctx.core, &ctx.tx, &state);
    }

    unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code == HC_ACTION as i32 {
            // SAFETY: for HC_ACTION, lparam points to a KBDLLHOOKSTRUCT valid for this call.
            let info = unsafe { &*(lparam as *const KBDLLHOOKSTRUCT) };
            on_key(wparam as u32, info.vkCode);
        }
        // SAFETY: passes the untouched arguments on, so every key still reaches the next hook.
        unsafe { CallNextHookEx(ptr::null_mut(), code, wparam, lparam) }
    }

    fn install_hook() -> HHOOK {
        // SAFETY: hook_proc matches HOOKPROC, a NULL module name means this executable.
        unsafe {
            let module = GetModuleHandleW(ptr::null());
            SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), module, 0)
        }
    }

    fn remove_hook(hook: HHOOK) {
        // SAFETY: `hook` came from `install_hook` on this thread and is removed once.
        unsafe { UnhookWindowsHookEx(hook) };
    }

    fn thread_id() -> u32 {
        // SAFETY: no preconditions.
        unsafe { GetCurrentThreadId() }
    }

    fn last_error() -> u32 {
        // SAFETY: no preconditions.
        unsafe { GetLastError() }
    }

    fn create_message_queue() {
        let mut msg = MSG::default();
        // SAFETY: `msg` is a valid MSG; PM_NOREMOVE only creates this thread's queue.
        unsafe { PeekMessageW(&mut msg, ptr::null_mut(), 0, 0, PM_NOREMOVE) };
    }

    fn pump_messages() {
        let mut msg = MSG::default();
        // SAFETY: `msg` is a valid MSG; GetMessageW returns 0 on WM_QUIT and -1 on error.
        while unsafe { GetMessageW(&mut msg, ptr::null_mut(), 0, 0) } > 0 {}
    }

    fn post_quit(thread: u32) {
        // SAFETY: posting to a thread id that may already be gone just fails.
        unsafe { PostThreadMessageW(thread, WM_QUIT, 0, 0) };
    }

    fn run(ctx: Ctx, ready: Sender<Result<u32, String>>) {
        {
            let mut slot = lock_ctx();
            if slot.is_some() {
                let _ = ready.send(Err("The keyboard hook is already running.".into()));
                return;
            }
            *slot = Some(ctx);
        }
        let hook = install_hook();
        if hook.is_null() {
            *lock_ctx() = None;
            let _ = ready.send(Err(format!(
                "Could not install the keyboard hook (Windows error {}).",
                last_error()
            )));
            return;
        }
        create_message_queue();
        let _ = ready.send(Ok(thread_id()));
        pump_messages();
        remove_hook(hook);
        *lock_ctx() = None;
    }

    // Stops the hook thread when dropped.
    pub struct Guard {
        thread_id: u32,
        thread: Option<JoinHandle<()>>,
    }

    impl Drop for Guard {
        fn drop(&mut self) {
            if let Some(thread) = self.thread.take() {
                // Repeat in case the queue was not ready for the first WM_QUIT.
                while !thread.is_finished() {
                    post_quit(self.thread_id);
                    thread::sleep(Duration::from_millis(5));
                }
                let _ = thread.join();
            }
        }
    }

    pub fn spawn(core: Arc<Mutex<Core>>, tx: Sender<HotkeyEvent>) -> Result<Guard, String> {
        let (ready_tx, ready_rx) = mpsc::channel();
        let ctx = Ctx {
            core,
            tx,
            keys: WinKeys::default(),
        };
        let thread = thread::Builder::new()
            .name("hotkey".into())
            .spawn(move || run(ctx, ready_tx))
            .map_err(|e| format!("Could not start the hotkey thread: {e}"))?;
        match ready_rx.recv() {
            Ok(Ok(thread_id)) => Ok(Guard {
                thread_id,
                thread: Some(thread),
            }),
            Ok(Err(message)) => {
                let _ = thread.join();
                Err(message)
            }
            Err(_) => Err("The hotkey thread stopped before it was ready.".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn left_and_right_modifiers_fold_together() {
        let mut keys = WinKeys::default();
        keys.update(VK_LCONTROL, true);
        let s = keys.update(VK_RMENU, true);
        assert_eq!(s.modifiers, BTreeSet::from([Modifier::Ctrl, Modifier::Alt]));
        keys.update(VK_RCONTROL, true);
        let s = keys.update(VK_LCONTROL, false);
        assert!(s.modifiers.contains(&Modifier::Ctrl));
        let s = keys.update(VK_RCONTROL, false);
        assert_eq!(s.modifiers, BTreeSet::from([Modifier::Alt]));
    }

    #[test]
    fn autorepeat_does_not_duplicate_and_key_up_clears() {
        let mut keys = WinKeys::default();
        keys.update(VK_LSHIFT, true);
        keys.update(VK_SPACE, true);
        let s = keys.update(VK_SPACE, true);
        assert_eq!(s.key.as_deref(), Some("Space"));
        let s = keys.update(VK_SPACE, false);
        assert_eq!(s.key, None);
        assert_eq!(s.modifiers, BTreeSet::from([Modifier::Shift]));
    }

    #[test]
    fn escape_is_not_a_key() {
        let mut keys = WinKeys::default();
        let s = keys.update(VK_ESCAPE, true);
        assert!(s.escape);
        assert_eq!(s.key, None);
        let s = keys.update(VK_ESCAPE, false);
        assert!(!s.escape);
    }

    #[test]
    fn key_names_follow_the_combo_alphabet() {
        assert_eq!(key_name(0x41), "A");
        assert_eq!(key_name(0x5A), "Z");
        assert_eq!(key_name(0x30), "0");
        assert_eq!(key_name(0x39), "9");
        assert_eq!(key_name(VK_F1), "F1");
        assert_eq!(key_name(VK_F24), "F24");
        assert_eq!(key_name(VK_SPACE), "Space");
        assert_eq!(key_name(0x25), "Other");
    }

    #[test]
    fn win_key_is_meta() {
        let mut keys = WinKeys::default();
        let s = keys.update(VK_LWIN, true);
        assert_eq!(s.modifiers, BTreeSet::from([Modifier::Meta]));
    }
}
