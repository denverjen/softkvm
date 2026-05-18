use softkvm_protocol::message::*;

#[cfg(target_os = "linux")]
pub struct InputInjector {
    keyboard: evdev::uinput::VirtualDevice,
    mouse: evdev::uinput::VirtualDevice,
}

#[cfg(target_os = "linux")]
impl InputInjector {
    pub fn new() -> anyhow::Result<Self> {
        use evdev::{AttributeSet, Key, RelativeAxisType, AbsoluteAxisType, AbsInfo, UinputAbsSetup, uinput::VirtualDeviceBuilder};

        let mut keys = AttributeSet::<Key>::new();
        for k in 0..256u16 {
            keys.insert(Key::new(k));
        }

        let keyboard = VirtualDeviceBuilder::new()?
            .name("SoftKVM Keyboard")
            .with_keys(&keys)?
            .build()?;

        let mut rel_axes = AttributeSet::<RelativeAxisType>::new();
        rel_axes.insert(RelativeAxisType::REL_X);
        rel_axes.insert(RelativeAxisType::REL_Y);
        rel_axes.insert(RelativeAxisType::REL_WHEEL);
        rel_axes.insert(RelativeAxisType::REL_HWHEEL);

        let mut mouse_keys = AttributeSet::<Key>::new();
        mouse_keys.insert(Key::BTN_LEFT);
        mouse_keys.insert(Key::BTN_RIGHT);
        mouse_keys.insert(Key::BTN_MIDDLE);
        mouse_keys.insert(Key::BTN_SIDE);
        mouse_keys.insert(Key::BTN_EXTRA);

        let abs_info = AbsInfo::new(0, 0, 65535, 0, 0, 0);

        let mouse = VirtualDeviceBuilder::new()?
            .name("SoftKVM Mouse")
            .with_keys(&mouse_keys)?
            .with_relative_axes(&rel_axes)?
            .with_absolute_axis(&UinputAbsSetup::new(AbsoluteAxisType::ABS_X, abs_info))?
            .with_absolute_axis(&UinputAbsSetup::new(AbsoluteAxisType::ABS_Y, abs_info))?
            .build()?;

        Ok(Self { keyboard, mouse })
    }

    pub fn mouse_move(&mut self, dx: i16, dy: i16) -> anyhow::Result<()> {
        let mut events = vec![];
        if dx != 0 {
            events.push(evdev::InputEvent::new(
                evdev::EventType::RELATIVE,
                evdev::RelativeAxisType::REL_X.0,
                dx as i32,
            ));
        }
        if dy != 0 {
            events.push(evdev::InputEvent::new(
                evdev::EventType::RELATIVE,
                evdev::RelativeAxisType::REL_Y.0,
                dy as i32,
            ));
        }
        if !events.is_empty() {
            events.push(evdev::InputEvent::new(
                evdev::EventType::SYNCHRONIZATION,
                evdev::Synchronization::SYN_REPORT.0,
                0,
            ));
            self.mouse.emit(&events)?;
        }
        Ok(())
    }

    pub fn mouse_move_absolute(&mut self, x: i32, y: i32, screen_w: i32, screen_h: i32) -> anyhow::Result<()> {
        use evdev::AbsoluteAxisType;
        let mapped_x = (x as f64 / screen_w as f64 * 65535.0) as i32;
        let mapped_y = (y as f64 / screen_h as f64 * 65535.0) as i32;
        self.mouse.emit(&[
            evdev::InputEvent::new(evdev::EventType::ABSOLUTE, AbsoluteAxisType::ABS_X.0, mapped_x),
            evdev::InputEvent::new(evdev::EventType::ABSOLUTE, AbsoluteAxisType::ABS_Y.0, mapped_y),
            evdev::InputEvent::new(
                evdev::EventType::SYNCHRONIZATION,
                evdev::Synchronization::SYN_REPORT.0,
                0,
            ),
        ])?;
        Ok(())
    }

    pub fn mouse_button(&mut self, button: MouseButtonId, state: ButtonState) -> anyhow::Result<()> {
        let key = match button {
            MouseButtonId::Left => evdev::Key::BTN_LEFT,
            MouseButtonId::Middle => evdev::Key::BTN_MIDDLE,
            MouseButtonId::Right => evdev::Key::BTN_RIGHT,
            MouseButtonId::Side1 => evdev::Key::BTN_SIDE,
            MouseButtonId::Side2 => evdev::Key::BTN_EXTRA,
        };
        let val = match state {
            ButtonState::Pressed => 1,
            ButtonState::Released => 0,
        };
        self.mouse.emit(&[
            evdev::InputEvent::new(evdev::EventType::KEY, key.0, val),
            evdev::InputEvent::new(
                evdev::EventType::SYNCHRONIZATION,
                evdev::Synchronization::SYN_REPORT.0,
                0,
            ),
        ])?;
        Ok(())
    }

    pub fn mouse_scroll(&mut self, delta: i16) -> anyhow::Result<()> {
        let val = if delta > 0 { 1 } else { -1 };
        self.mouse.emit(&[
            evdev::InputEvent::new(
                evdev::EventType::RELATIVE,
                evdev::RelativeAxisType::REL_WHEEL.0,
                val,
            ),
            evdev::InputEvent::new(
                evdev::EventType::SYNCHRONIZATION,
                evdev::Synchronization::SYN_REPORT.0,
                0,
            ),
        ])?;
        Ok(())
    }

    pub fn key(&mut self, keycode: u16, pressed: bool) -> anyhow::Result<()> {
        let evdev_code = vk_to_evdev(keycode);
        let val = if pressed { 1 } else { 0 };
        self.keyboard.emit(&[
            evdev::InputEvent::new(evdev::EventType::KEY, evdev_code, val),
            evdev::InputEvent::new(
                evdev::EventType::SYNCHRONIZATION,
                evdev::Synchronization::SYN_REPORT.0,
                0,
            ),
        ])?;
        Ok(())
    }
}

#[cfg(target_os = "linux")]
fn vk_to_evdev(vk: u16) -> u16 {
    match vk {
        0x08 => 0x0E,
        0x09 => 0x0F,
        0x0D => 0x1C,
        0x10 => 0x2A,
        0x11 => 0x1D,
        0x12 => 0x38,
        0x13 => 0x46,
        0x14 => 0x3A,
        0x1B => 0x01,
        0x20 => 0x39,
        0x21 => 0x49,
        0x22 => 0x51,
        0x23 => 0x4F,
        0x24 => 0x4A,
        0x25 => 0x50,
        0x26 => 0x4C,
        0x27 => 0x4D,
        0x28 => 0x4E,
        0x2C => 0x54,
        0x2D => 0x52,
        0x2E => 0x53,
        0x30 => 0x0B,
        0x31 => 0x02,
        0x32 => 0x03,
        0x33 => 0x04,
        0x34 => 0x05,
        0x35 => 0x06,
        0x36 => 0x07,
        0x37 => 0x08,
        0x38 => 0x09,
        0x39 => 0x0A,
        0x41 => 0x1E,
        0x42 => 0x30,
        0x43 => 0x2E,
        0x44 => 0x20,
        0x45 => 0x12,
        0x46 => 0x21,
        0x47 => 0x22,
        0x48 => 0x23,
        0x49 => 0x17,
        0x4A => 0x24,
        0x4B => 0x25,
        0x4C => 0x26,
        0x4D => 0x32,
        0x4E => 0x31,
        0x4F => 0x18,
        0x50 => 0x19,
        0x51 => 0x10,
        0x52 => 0x13,
        0x53 => 0x1F,
        0x54 => 0x14,
        0x55 => 0x16,
        0x56 => 0x2F,
        0x57 => 0x11,
        0x58 => 0x2D,
        0x59 => 0x15,
        0x5A => 0x2C,
        0x5B => 0x7D,
        0x5C => 0x7E,
        0x5D => 0x7F,
        0x60 => 0x52,
        0x61 => 0x4F,
        0x62 => 0x50,
        0x63 => 0x51,
        0x64 => 0x4B,
        0x65 => 0x4C,
        0x66 => 0x4D,
        0x67 => 0x47,
        0x68 => 0x48,
        0x69 => 0x49,
        0x6A => 0x37,
        0x6B => 0x4E,
        0x6D => 0x4A,
        0x6E => 0x53,
        0x6F => 0x57,
        0x70 => 0x3B,
        0x71 => 0x3C,
        0x72 => 0x3D,
        0x73 => 0x3E,
        0x74 => 0x3F,
        0x75 => 0x40,
        0x76 => 0x41,
        0x77 => 0x42,
        0x78 => 0x43,
        0x79 => 0x44,
        0x7A => 0x45,
        0x7B => 0x46,
        0x7C => 0x47,
        0x7D => 0x48,
        0x7E => 0x49,
        0x7F => 0x5A,
        0x80 => 0x5B,
        0x81 => 0x5C,
        0x82 => 0x5D,
        0x83 => 0x5E,
        0x84 => 0x5F,
        0x85 => 0x60,
        0x86 => 0x61,
        0x87 => 0x62,
        0x90 => 0x45,
        0x91 => 0x46,
        0xA0 => 0x2A,
        0xA1 => 0x36,
        0xA2 => 0x1D,
        0xA3 => 0x61,
        0xA4 => 0x38,
        0xA5 => 0x64,
        0xBA => 0x27,
        0xBB => 0x0D,
        0xBC => 0x33,
        0xBD => 0x0C,
        0xBE => 0x34,
        0xBF => 0x35,
        0xC0 => 0x29,
        0xDB => 0x1A,
        0xDC => 0x2B,
        0xDD => 0x1B,
        0xDE => 0x28,
        code => code,
    }
}

#[cfg(target_os = "windows")]
pub struct InputInjector;

#[cfg(target_os = "windows")]
impl InputInjector {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self)
    }

    pub fn mouse_move_absolute(&mut self, _x: i32, _y: i32, _screen_w: i32, _screen_h: i32) -> anyhow::Result<()> {
        Ok(())
    }

    pub fn mouse_move(&mut self, dx: i16, dy: i16) -> anyhow::Result<()> {
        use windows::Win32::UI::Input::KeyboardAndMouse::*;

        let input = INPUT {
            r#type: INPUT_TYPE(0),
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: dx as i32,
                    dy: dy as i32,
                    mouseData: 0,
                    dwFlags: MOUSE_EVENT_FLAGS(0x0001),
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        unsafe {
            SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
        }
        Ok(())
    }

    pub fn mouse_button(&mut self, button: MouseButtonId, state: ButtonState) -> anyhow::Result<()> {
        use windows::Win32::UI::Input::KeyboardAndMouse::*;

        let flag = match (button, state) {
            (MouseButtonId::Left, ButtonState::Pressed) => MOUSE_EVENT_FLAGS(0x0002),
            (MouseButtonId::Left, ButtonState::Released) => MOUSE_EVENT_FLAGS(0x0004),
            (MouseButtonId::Right, ButtonState::Pressed) => MOUSE_EVENT_FLAGS(0x0008),
            (MouseButtonId::Right, ButtonState::Released) => MOUSE_EVENT_FLAGS(0x0010),
            (MouseButtonId::Middle, ButtonState::Pressed) => MOUSE_EVENT_FLAGS(0x0020),
            (MouseButtonId::Middle, ButtonState::Released) => MOUSE_EVENT_FLAGS(0x0040),
            _ => return Ok(()),
        };

        let input = INPUT {
            r#type: INPUT_TYPE(0),
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: 0,
                    dy: 0,
                    mouseData: 0,
                    dwFlags: flag,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        unsafe {
            SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
        }
        Ok(())
    }

    pub fn mouse_scroll(&mut self, delta: i16) -> anyhow::Result<()> {
        use windows::Win32::UI::Input::KeyboardAndMouse::*;

        let input = INPUT {
            r#type: INPUT_TYPE(0),
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: 0,
                    dy: 0,
                    mouseData: ((delta as i32) * 120) as u32,
                    dwFlags: MOUSE_EVENT_FLAGS(0x0800),
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        unsafe {
            SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
        }
        Ok(())
    }

    pub fn key(&mut self, keycode: u16, pressed: bool) -> anyhow::Result<()> {
        use windows::Win32::UI::Input::KeyboardAndMouse::*;

        let flag = if pressed {
            KEYBD_EVENT_FLAGS(0)
        } else {
            KEYEVENTF_KEYUP
        };

        let input = INPUT {
            r#type: INPUT_TYPE(1),
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(keycode),
                    wScan: 0,
                    dwFlags: flag,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        unsafe {
            SendInput(&[input], std::mem::size_of::<INPUT>() as i32);
        }
        Ok(())
    }
}
