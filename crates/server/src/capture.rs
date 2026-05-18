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
pub fn start_capture(tx: mpsc::Sender<CaptureEvent>) -> anyhow::Result<()> {
    use std::sync::atomic::{AtomicI32, Ordering};
    use std::sync::Arc;
    use windows::Win32::Foundation::*;
    use windows::Win32::UI::Input::*;
    use windows::Win32::UI::WindowsAndMessaging::*;

    let cursor_x: Arc<AtomicI32> = Arc::new(AtomicI32::new(0));
    let cursor_y: Arc<AtomicI32> = Arc::new(AtomicI32::new(0));

    std::thread::spawn(move || -> anyhow::Result<()> {
        unsafe {
            let rid_keyboard = RAWINPUTDEVICE {
                usUsagePage: 0x01,
                usUsage: 0x06,
                dwFlags: RIDEV_INPUTSINK,
                hwndTarget: HWND(std::ptr::null_mut()),
            };
            RegisterRawInputDevices(&[rid_keyboard], std::mem::size_of::<RAWINPUTDEVICE>() as u32);

            let rid_mouse = RAWINPUTDEVICE {
                usUsagePage: 0x01,
                usUsage: 0x02,
                dwFlags: RIDEV_INPUTSINK,
                hwndTarget: HWND(std::ptr::null_mut()),
            };
            RegisterRawInputDevices(&[rid_mouse], std::mem::size_of::<RAWINPUTDEVICE>() as u32);
        }

        let mut msg = MSG::default();
        unsafe {
            while GetMessageW(&mut msg, HWND(std::ptr::null_mut()), 0, 0).as_bool() {
                if msg.message == WM_INPUT {
                    let mut point = POINT::default();
                    windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut point);
                    let x = point.x;
                    let y = point.y;
                    cursor_x.store(x, Ordering::SeqCst);
                    cursor_y.store(y, Ordering::SeqCst);
                    let _ = tx.blocking_send(CaptureEvent {
                        message: Message::Heartbeat,
                        abs_x: x,
                        abs_y: y,
                    });
                }
                DispatchMessageW(&msg);
            }
        }
        Ok(())
    });
    Ok(())
}
