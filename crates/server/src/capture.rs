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
    tracing::info!("Capture started, screen: {}x{}", screen_w, screen_h);

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
                                                x: 0,
                                                y: 0,
                                            }))
                                        }
                                        RelativeAxisType::REL_Y => {
                                            let dy = ev.value();
                                            cursor_y.fetch_add(dy, Ordering::SeqCst);
                                            Some(Message::MouseMove(MouseMovePayload {
                                                x: 0,
                                                y: 0,
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
pub fn set_block_mouse(_block: bool) {}

#[cfg(target_os = "windows")]
pub fn start_capture(tx: mpsc::Sender<CaptureEvent>) -> anyhow::Result<()> {
    use softkvm_protocol::message::*;
    use windows::Win32::Foundation::*;
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::Input::*;
    use windows::Win32::UI::WindowsAndMessaging::*;

    let tx = std::sync::Mutex::new(Some(tx));

    std::thread::spawn(move || -> anyhow::Result<()> {
        unsafe {
            let instance = GetModuleHandleW(None)?;
            let class_name = windows::core::w!("SoftKVMCapture");

            unsafe extern "system" fn default_wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }

            let wc = WNDCLASSW {
                lpfnWndProc: Some(default_wnd_proc),
                hInstance: instance.into(),
                lpszClassName: class_name,
                ..Default::default()
            };
            RegisterClassW(&wc);

            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                class_name,
                windows::core::w!("SoftKVM"),
                WINDOW_STYLE::default(),
                0, 0, 0, 0,
                HWND::default(),
                HMENU::default(),
                instance,
                None,
            )?;
            tracing::info!("Raw input hidden window created");

            let devices = [
                RAWINPUTDEVICE {
                    usUsagePage: 0x01,
                    usUsage: 0x06,
                    dwFlags: RIDEV_INPUTSINK,
                    hwndTarget: hwnd,
                },
                RAWINPUTDEVICE {
                    usUsagePage: 0x01,
                    usUsage: 0x02,
                    dwFlags: RIDEV_INPUTSINK,
                    hwndTarget: hwnd,
                },
            ];
            match RegisterRawInputDevices(&devices, std::mem::size_of::<RAWINPUTDEVICE>() as u32) {
                Ok(()) => tracing::info!("Raw input devices registered"),
                Err(e) => tracing::error!("RegisterRawInputDevices FAILED: {}", e),
            }

            let hook_instance = GetModuleHandleW(None)?;
            let _mouse_hook = SetWindowsHookExW(
                WH_KEYBOARD_LL,
                Some(keyboard_hook_callback),
                hook_instance,
                0,
            )?;
            tracing::info!("Keyboard low-level hook installed");

            let mut msg = MSG::default();
            while GetMessageW(&mut msg, HWND::default(), 0, 0).0 > 0 {
                if msg.message == WM_INPUT {
                    let mut raw: RAWINPUT = std::mem::zeroed();
                    let mut size = std::mem::size_of::<RAWINPUT>() as u32;
                    let result = GetRawInputData(
                        HRAWINPUT(msg.lParam.0 as *mut _),
                        RID_INPUT,
                        Some(&mut raw as *mut RAWINPUT as *mut core::ffi::c_void),
                        &mut size,
                        std::mem::size_of::<RAWINPUTHEADER>() as u32,
                    );
                    if result == 0 || result == u32::MAX {
                        DispatchMessageW(&msg);
                        continue;
                    }

                    let guard = tx.lock().unwrap();
                    if let Some(tx) = guard.as_ref() {
                        let device_type = raw.header.dwType;
                        if device_type == RIM_TYPEMOUSE.0 {
                            let mouse = raw.data.mouse;
                            let dx = mouse.lLastX as i16;
                            let dy = mouse.lLastY as i16;

                            if dx != 0 || dy != 0 {
                                let mut point = POINT::default();
                                let _ = GetCursorPos(&mut point);

                                let _ = tx.blocking_send(CaptureEvent {
                                    message: Message::MouseMove(MouseMovePayload { x: 0, y: 0 }),
                                    abs_x: point.x,
                                    abs_y: point.y,
                                });
                            }

                            let button_flags = mouse.Anonymous.Anonymous.usButtonFlags;
                            if button_flags != 0 {
                                let mut point = POINT::default();
                                let _ = GetCursorPos(&mut point);

                                if button_flags & RI_MOUSE_LEFT_BUTTON_DOWN as u16 != 0 {
                                    let _ = tx.blocking_send(CaptureEvent {
                                        message: Message::MouseButton(MouseButtonPayload {
                                            button: MouseButtonId::Left,
                                            state: ButtonState::Pressed,
                                        }),
                                        abs_x: point.x,
                                        abs_y: point.y,
                                    });
                                }
                                if button_flags & RI_MOUSE_LEFT_BUTTON_UP as u16 != 0 {
                                    let _ = tx.blocking_send(CaptureEvent {
                                        message: Message::MouseButton(MouseButtonPayload {
                                            button: MouseButtonId::Left,
                                            state: ButtonState::Released,
                                        }),
                                        abs_x: point.x,
                                        abs_y: point.y,
                                    });
                                }
                                if button_flags & RI_MOUSE_RIGHT_BUTTON_DOWN as u16 != 0 {
                                    let _ = tx.blocking_send(CaptureEvent {
                                        message: Message::MouseButton(MouseButtonPayload {
                                            button: MouseButtonId::Right,
                                            state: ButtonState::Pressed,
                                        }),
                                        abs_x: point.x,
                                        abs_y: point.y,
                                    });
                                }
                                if button_flags & RI_MOUSE_RIGHT_BUTTON_UP as u16 != 0 {
                                    let _ = tx.blocking_send(CaptureEvent {
                                        message: Message::MouseButton(MouseButtonPayload {
                                            button: MouseButtonId::Right,
                                            state: ButtonState::Released,
                                        }),
                                        abs_x: point.x,
                                        abs_y: point.y,
                                    });
                                }
                                if button_flags & RI_MOUSE_MIDDLE_BUTTON_DOWN as u16 != 0 {
                                    let _ = tx.blocking_send(CaptureEvent {
                                        message: Message::MouseButton(MouseButtonPayload {
                                            button: MouseButtonId::Middle,
                                            state: ButtonState::Pressed,
                                        }),
                                        abs_x: point.x,
                                        abs_y: point.y,
                                    });
                                }
                                if button_flags & RI_MOUSE_MIDDLE_BUTTON_UP as u16 != 0 {
                                    let _ = tx.blocking_send(CaptureEvent {
                                        message: Message::MouseButton(MouseButtonPayload {
                                            button: MouseButtonId::Middle,
                                            state: ButtonState::Released,
                                        }),
                                        abs_x: point.x,
                                        abs_y: point.y,
                                    });
                                }
                                if button_flags & RI_MOUSE_WHEEL as u16 != 0 {
                                    let delta = mouse.Anonymous.Anonymous.usButtonData as i16;
                                    let _ = tx.blocking_send(CaptureEvent {
                                        message: Message::MouseScroll(MouseScrollPayload { delta }),
                                        abs_x: point.x,
                                        abs_y: point.y,
                                    });
                                }
                            }
                        } else if device_type == RIM_TYPEKEYBOARD.0 {
                            let keyboard = raw.data.keyboard;
                            let vkey = keyboard.VKey as u16;
                            let flags = keyboard.Flags;
                            let pressed = (flags as u32 & RI_KEY_BREAK) == 0;

                            let mut point = POINT::default();
                            let _ = GetCursorPos(&mut point);
                            let msg = if pressed {
                                Message::KeyDown(KeyPayload { keycode: vkey })
                            } else {
                                Message::KeyUp(KeyPayload { keycode: vkey })
                            };
                            let _ = tx.blocking_send(CaptureEvent {
                                message: msg,
                                abs_x: point.x,
                                abs_y: point.y,
                            });
                        }
                    }
                }
                DispatchMessageW(&msg);
            }
        }
        Ok(())
    });
    Ok(())
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn keyboard_hook_callback(
    code: i32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::UI::WindowsAndMessaging::*;
    CallNextHookEx(HHOOK::default(), code, wparam, lparam)
}
