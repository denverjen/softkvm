use softkvm_protocol::Message;
use tokio::sync::mpsc;

#[cfg(target_os = "windows")]
pub fn start_capture(tx: mpsc::Sender<Message>) -> anyhow::Result<()> {
    use windows::Win32::Foundation::*;
    use windows::Win32::UI::Input::*;
    use windows::Win32::UI::WindowsAndMessaging::*;

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
                    let _ = tx.blocking_send(Message::Heartbeat);
                }
                DispatchMessageW(&msg);
            }
        }
        Ok(())
    });
    Ok(())
}

#[cfg(target_os = "linux")]
pub fn start_capture(tx: mpsc::Sender<Message>) -> anyhow::Result<()> {
    use evdev::{Device, EventType, InputEventKind};
    use softkvm_protocol::message::*;

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
                            match ev.kind() {
                                InputEventKind::Key(key) => {
                                    let keycode = key.0;
                                    let pressed = ev.value() != 0;
                                    let msg = if pressed {
                                        Message::KeyDown(KeyPayload { keycode })
                                    } else {
                                        Message::KeyUp(KeyPayload { keycode })
                                    };
                                    let _ = tx.blocking_send(msg);
                                }
                                InputEventKind::RelAxis(axis) => {
                                    use evdev::RelativeAxisType;
                                    match axis {
                                        RelativeAxisType::REL_X => {
                                            let _ = tx.blocking_send(Message::MouseMove(MouseMovePayload {
                                                dx: ev.value() as i16,
                                                dy: 0,
                                            }));
                                        }
                                        RelativeAxisType::REL_Y => {
                                            let _ = tx.blocking_send(Message::MouseMove(MouseMovePayload {
                                                dx: 0,
                                                dy: ev.value() as i16,
                                            }));
                                        }
                                        RelativeAxisType::REL_WHEEL => {
                                            let _ = tx.blocking_send(Message::MouseScroll(MouseScrollPayload {
                                                delta: ev.value() as i16,
                                            }));
                                        }
                                        _ => {}
                                    }
                                }
                                _ => {}
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

pub fn vk_to_keycode(vk: u32) -> u16 {
    vk as u16
}
