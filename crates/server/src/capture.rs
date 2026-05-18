use softkvm_protocol::Message;
use tokio::sync::mpsc;

pub struct CaptureEvent {
    pub message: Message,
    pub abs_x: i32,
    pub abs_y: i32,
}

#[cfg(target_os = "linux")]
pub fn start_capture(tx: mpsc::Sender<CaptureEvent>) -> anyhow::Result<()> {
    use evdev::{Device, EventType, InputEventKind};
    use softkvm_protocol::message::*;
    use std::sync::atomic::{AtomicI32, Ordering};
    use std::sync::Arc;

    let cursor_x: Arc<AtomicI32> = Arc::new(AtomicI32::new(0));
    let cursor_y: Arc<AtomicI32> = Arc::new(AtomicI32::new(0));

    let (screen_w, screen_h) = get_screen_size();
    cursor_x.store(screen_w as i32 / 2, Ordering::SeqCst);
    cursor_y.store(screen_h as i32 / 2, Ordering::SeqCst);
    tracing::info!("Capture started, screen: {}x{}, cursor init: ({}, {})", screen_w, screen_h, screen_w / 2, screen_h / 2);

    std::thread::spawn(move || -> anyhow::Result<()> {
        let mut devices: Vec<Device> = evdev::enumerate()
            .filter_map(|(_, d)| {
                let supported = d.supported_events();
                if supported.contains(EventType::KEY) || supported.contains(EventType::RELATIVE) {
                    Some(d)
                } else {
                    None
                }
            })
            .collect();

        if devices.is_empty() {
            tracing::error!("No input devices found for capture");
            return Ok(());
        }

        tracing::info!("Found {} input devices for capture", devices.len());

        loop {
            for device in &mut devices {
                match device.fetch_events() {
                    Ok(events) => {
                        for ev in events {
                            let msg = match ev.kind() {
                                InputEventKind::Key(key) => {
                                    let keycode = key.0;
                                    let pressed = ev.value() != 0;
                                    if pressed {
                                        Some(Message::KeyDown(KeyPayload { keycode }))
                                    } else {
                                        Some(Message::KeyUp(KeyPayload { keycode }))
                                    }
                                }
                                InputEventKind::RelAxis(axis) => {
                                    use evdev::RelativeAxisType;
                                    match axis {
                                        RelativeAxisType::REL_X => {
                                            let dx = ev.value();
                                            cursor_x.fetch_add(dx, Ordering::SeqCst);
                                            Some(Message::MouseMove(MouseMovePayload {
                                                dx: dx as i16,
                                                dy: 0,
                                            }))
                                        }
                                        RelativeAxisType::REL_Y => {
                                            let dy = ev.value();
                                            cursor_y.fetch_add(dy, Ordering::SeqCst);
                                            Some(Message::MouseMove(MouseMovePayload {
                                                dx: 0,
                                                dy: dy as i16,
                                            }))
                                        }
                                        RelativeAxisType::REL_WHEEL => {
                                            Some(Message::MouseScroll(MouseScrollPayload {
                                                delta: ev.value() as i16,
                                            }))
                                        }
                                        _ => None,
                                    }
                                }
                                _ => None,
                            };

                            if let Some(message) = msg {
                                let x = cursor_x.load(Ordering::SeqCst);
                                let y = cursor_y.load(Ordering::SeqCst);
                                let _ = tx.blocking_send(CaptureEvent {
                                    message,
                                    abs_x: x,
                                    abs_y: y,
                                });
                            }
                        }
                    }
                    Err(e) => {
                        if e.kind() != std::io::ErrorKind::WouldBlock {
                            tracing::warn!("Device read error: {}", e);
                        }
                    }
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    });
    Ok(())
}

#[cfg(target_os = "linux")]
fn get_screen_size() -> (u32, u32) {
    use x11::xlib::{XOpenDisplay, XDefaultScreenOfDisplay};

    unsafe {
        let display = XOpenDisplay(std::ptr::null());
        if display.is_null() {
            tracing::warn!("Cannot open X display, using default 1920x1080");
            return (1920, 1080);
        }

        let screen = XDefaultScreenOfDisplay(display);
        let width = (*screen).width as u32;
        let height = (*screen).height as u32;

        x11::xlib::XCloseDisplay(display);
        (width, height)
    }
}

#[cfg(target_os = "windows")]
static TX: std::sync::Mutex<Option<mpsc::Sender<CaptureEvent>>> = std::sync::Mutex::new(None);

#[cfg(target_os = "windows")]
static PREV_POS: std::sync::Mutex<Option<(i32, i32)>> = std::sync::Mutex::new(None);

#[cfg(target_os = "windows")]
static BLOCK_MOUSE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[cfg(target_os = "windows")]
pub fn set_block_mouse(block: bool) {
    BLOCK_MOUSE.store(block, std::sync::atomic::Ordering::Release);
}

#[cfg(target_os = "windows")]
pub fn start_capture(tx: mpsc::Sender<CaptureEvent>) -> anyhow::Result<()> {
    use windows::Win32::Foundation::*;
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::*;

    {
        let mut guard = TX.lock().unwrap();
        *guard = Some(tx);
    }

    std::thread::spawn(move || -> anyhow::Result<()> {
        unsafe {
            let instance = GetModuleHandleW(None)?;

            let mouse_hook = SetWindowsHookExW(
                WH_MOUSE_LL,
                Some(mouse_hook_callback),
                instance,
                0,
            )?;
            tracing::info!("Mouse low-level hook installed");

            let keyboard_hook = SetWindowsHookExW(
                WH_KEYBOARD_LL,
                Some(keyboard_hook_callback),
                instance,
                0,
            )?;
            tracing::info!("Keyboard low-level hook installed");

            let mut msg = MSG::default();
            while GetMessageW(&mut msg, HWND::default(), 0, 0).0 > 0 {
                DispatchMessageW(&msg);
            }

            let _ = UnhookWindowsHookEx(mouse_hook);
            let _ = UnhookWindowsHookEx(keyboard_hook);
        }
        Ok(())
    });
    Ok(())
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn mouse_hook_callback(
    code: i32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::Foundation::*;
    use windows::Win32::UI::WindowsAndMessaging::*;
    use softkvm_protocol::message::*;

    if code >= 0 {
        let llhs = &*(lparam.0 as *const MSLLHOOKSTRUCT);
        let block = crate::capture::BLOCK_MOUSE.load(std::sync::atomic::Ordering::Acquire);
        let guard = crate::capture::TX.lock().unwrap();
        if let Some(tx) = guard.as_ref() {
            let msg_type = wparam.0 as u32;
            let abs_x = llhs.pt.x;
            let abs_y = llhs.pt.y;

            match msg_type {
                x if x == WM_MOUSEMOVE => {
                    let mut prev = crate::capture::PREV_POS.lock().unwrap();
                    let (dx, dy) = if let Some((px, py)) = *prev {
                        ((abs_x - px) as i16, (abs_y - py) as i16)
                    } else {
                        (0i16, 0i16)
                    };
                    *prev = Some((abs_x, abs_y));
                    drop(prev);

                    if block {
                        if dx != 0 || dy != 0 {
                            if dx != 0 {
                                let _ = tx.blocking_send(CaptureEvent {
                                    message: Message::MouseMove(MouseMovePayload { dx, dy: 0 }),
                                    abs_x,
                                    abs_y,
                                });
                            }
                            if dy != 0 {
                                let _ = tx.blocking_send(CaptureEvent {
                                    message: Message::MouseMove(MouseMovePayload { dx: 0, dy }),
                                    abs_x,
                                    abs_y,
                                });
                            }
                        }
                        drop(guard);
                        return LRESULT(1);
                    }

                    if dx != 0 {
                        let _ = tx.blocking_send(CaptureEvent {
                            message: Message::MouseMove(MouseMovePayload { dx, dy: 0 }),
                            abs_x,
                            abs_y,
                        });
                    }
                    if dy != 0 {
                        let _ = tx.blocking_send(CaptureEvent {
                            message: Message::MouseMove(MouseMovePayload { dx: 0, dy }),
                            abs_x,
                            abs_y,
                        });
                    }
                }
                x if x == WM_LBUTTONDOWN => {
                    let _ = tx.blocking_send(CaptureEvent {
                        message: Message::MouseButton(MouseButtonPayload {
                            button: MouseButtonId::Left,
                            state: ButtonState::Pressed,
                        }),
                        abs_x,
                        abs_y,
                    });
                }
                x if x == WM_LBUTTONUP => {
                    let _ = tx.blocking_send(CaptureEvent {
                        message: Message::MouseButton(MouseButtonPayload {
                            button: MouseButtonId::Left,
                            state: ButtonState::Released,
                        }),
                        abs_x,
                        abs_y,
                    });
                }
                x if x == WM_RBUTTONDOWN => {
                    let _ = tx.blocking_send(CaptureEvent {
                        message: Message::MouseButton(MouseButtonPayload {
                            button: MouseButtonId::Right,
                            state: ButtonState::Pressed,
                        }),
                        abs_x,
                        abs_y,
                    });
                }
                x if x == WM_RBUTTONUP => {
                    let _ = tx.blocking_send(CaptureEvent {
                        message: Message::MouseButton(MouseButtonPayload {
                            button: MouseButtonId::Right,
                            state: ButtonState::Released,
                        }),
                        abs_x,
                        abs_y,
                    });
                }
                x if x == WM_MBUTTONDOWN => {
                    let _ = tx.blocking_send(CaptureEvent {
                        message: Message::MouseButton(MouseButtonPayload {
                            button: MouseButtonId::Middle,
                            state: ButtonState::Pressed,
                        }),
                        abs_x,
                        abs_y,
                    });
                }
                x if x == WM_MBUTTONUP => {
                    let _ = tx.blocking_send(CaptureEvent {
                        message: Message::MouseButton(MouseButtonPayload {
                            button: MouseButtonId::Middle,
                            state: ButtonState::Released,
                        }),
                        abs_x,
                        abs_y,
                    });
                }
                x if x == WM_MOUSEWHEEL => {
                    let delta = (llhs.mouseData >> 16) as i16;
                    let _ = tx.blocking_send(CaptureEvent {
                        message: Message::MouseScroll(MouseScrollPayload { delta }),
                        abs_x,
                        abs_y,
                    });
                }
                _ => {}
            }
        }
    }
    CallNextHookEx(HHOOK::default(), code, wparam, lparam)
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn keyboard_hook_callback(
    code: i32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::Foundation::*;
    use windows::Win32::UI::WindowsAndMessaging::*;
    use softkvm_protocol::message::*;

    if code >= 0 {
        let llhs = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
        let guard = crate::capture::TX.lock().unwrap();
        if let Some(tx) = guard.as_ref() {
            let msg_type = wparam.0 as u32;
            let vk = llhs.vkCode as u16;
            let pressed = msg_type == WM_KEYDOWN || msg_type == WM_SYSKEYDOWN;

            let msg = if pressed {
                Message::KeyDown(KeyPayload { keycode: vk })
            } else {
                Message::KeyUp(KeyPayload { keycode: vk })
            };

            let mut point = POINT::default();
            let _ = GetCursorPos(&mut point);
            let _ = tx.blocking_send(CaptureEvent {
                message: msg,
                abs_x: point.x,
                abs_y: point.y,
            });
        }
    }
    CallNextHookEx(HHOOK::default(), code, wparam, lparam)
}
