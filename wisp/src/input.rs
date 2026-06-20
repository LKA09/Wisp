//! Raw character-by-character console input (Windows).
//! Returns None from `try_read_key` when no input is available, so callers can
//! poll resize events and sleep between reads without blocking indefinitely.

#[allow(dead_code)]
pub enum Key {
    Char(char),
    Enter,
    Backspace,
    Escape,
}

#[cfg(windows)]
pub use windows_impl::RawConsole;

#[cfg(not(windows))]
pub use fallback::RawConsole;

// ─── Windows implementation ───────────────────────────────────────────────────

#[cfg(windows)]
mod windows_impl {
    use super::Key;
    use std::ffi::c_void;

    const STD_INPUT_HANDLE: u32 = 0xFFFF_FFF6;
    const KEY_EVENT: u16 = 0x0001;
    const VK_RETURN: u16 = 0x0D;
    const VK_BACK: u16 = 0x08;
    const VK_ESCAPE: u16 = 0x1B;
    const ENABLE_LINE_INPUT: u32 = 0x0002;
    const ENABLE_ECHO_INPUT: u32 = 0x0004;

    // INPUT_RECORD: 2 (event_type) + 2 (pad) + 16 (union, largest = KEY_EVENT_RECORD)
    #[repr(C)]
    struct InputRecord {
        event_type: u16,
        _pad: u16,
        _event: [u8; 16],
    }

    // KEY_EVENT_RECORD layout (offsets):
    //  0: BOOL  bKeyDown       (4)
    //  4: WORD  wRepeatCount   (2)
    //  6: WORD  wVirtualKeyCode(2)
    //  8: WORD  wVirtualScanCode(2)
    // 10: WCHAR uChar          (2)
    // 12: DWORD dwControlKeyState(4)
    fn parse_key_event(e: &[u8; 16]) -> (bool, u16, u16) {
        let key_down = u32::from_le_bytes([e[0], e[1], e[2], e[3]]) != 0;
        let vk = u16::from_le_bytes([e[6], e[7]]);
        let uc = u16::from_le_bytes([e[10], e[11]]);
        (key_down, vk, uc)
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetStdHandle(n: u32) -> *mut c_void;
        fn GetConsoleMode(h: *mut c_void, mode: *mut u32) -> i32;
        fn SetConsoleMode(h: *mut c_void, mode: u32) -> i32;
        fn GetNumberOfConsoleInputEvents(h: *mut c_void, count: *mut u32) -> i32;
        fn ReadConsoleInputW(
            h: *mut c_void,
            buf: *mut InputRecord,
            len: u32,
            read: *mut u32,
        ) -> i32;
    }

    pub struct RawConsole {
        handle: *mut c_void,
        original_mode: u32,
    }

    // RawConsole is used from one thread only.
    unsafe impl Send for RawConsole {}

    impl RawConsole {
        pub fn new() -> Option<Self> {
            unsafe {
                let h = GetStdHandle(STD_INPUT_HANDLE);
                if h.is_null() || h as usize == usize::MAX {
                    return None;
                }
                let mut mode = 0u32;
                if GetConsoleMode(h, &mut mode) == 0 {
                    return None;
                }
                let raw = mode & !(ENABLE_LINE_INPUT | ENABLE_ECHO_INPUT);
                if SetConsoleMode(h, raw) == 0 {
                    return None;
                }
                Some(RawConsole {
                    handle: h,
                    original_mode: mode,
                })
            }
        }

        /// Non-blocking: returns `Some(key)` if a key is ready, `None` if the
        /// input queue is empty.  Non-key events are consumed and skipped.
        pub fn try_read_key(&self) -> Option<Key> {
            loop {
                unsafe {
                    let mut count = 0u32;
                    if GetNumberOfConsoleInputEvents(self.handle, &mut count) == 0 || count == 0 {
                        return None; // queue empty
                    }

                    let mut record = InputRecord {
                        event_type: 0,
                        _pad: 0,
                        _event: [0u8; 16],
                    };
                    let mut read = 0u32;
                    if ReadConsoleInputW(self.handle, &mut record, 1, &mut read) == 0 || read == 0 {
                        return None;
                    }

                    // Skip non-key events (mouse, focus, resize…)
                    if record.event_type != KEY_EVENT {
                        continue;
                    }

                    let (key_down, vk, uc) = parse_key_event(&record._event);
                    if !key_down {
                        continue; // key-up events
                    }

                    return Some(match vk {
                        VK_RETURN => Key::Enter,
                        VK_BACK => Key::Backspace,
                        VK_ESCAPE => Key::Escape,
                        _ => {
                            // Ctrl+C → ETX (0x03)
                            if uc == 0x03 {
                                return Some(Key::Escape);
                            }
                            match char::from_u32(uc as u32) {
                                Some(c) if !c.is_control() => Key::Char(c),
                                _ => continue,
                            }
                        }
                    });
                }
            }
        }
    }

    impl Drop for RawConsole {
        fn drop(&mut self) {
            unsafe {
                SetConsoleMode(self.handle, self.original_mode);
            }
        }
    }
}

// ─── Non-Windows stub ─────────────────────────────────────────────────────────

#[cfg(not(windows))]
mod fallback {
    use super::Key;

    pub struct RawConsole;

    impl RawConsole {
        pub fn new() -> Option<Self> {
            None
        }
        pub fn try_read_key(&self) -> Option<Key> {
            None
        }
    }
}
