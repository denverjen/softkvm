use softkvm_protocol::message::*;

#[cfg(target_os = "linux")]
pub struct InputInjector {
    keyboard: evdev::uinput::VirtualDevice,
    mouse: evdev::uinput::VirtualDevice,
}

#[cfg(target_os = "linux")]
impl InputInjector {
    pub fn new() -> anyhow::Result<Self> {
        use evdev::{AttributeSet, Key, RelativeAxisType, uinput::VirtualDeviceBuilder};

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

        let mouse = VirtualDeviceBuilder::new()?
            .name("SoftKVM Mouse")
            .with_keys(&mouse_keys)?
            .with_relative_axes(&rel_axes)?
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
        let val = if pressed { 1 } else { 0 };
        self.keyboard.emit(&[
            evdev::InputEvent::new(evdev::EventType::KEY, keycode, val),
            evdev::InputEvent::new(
                evdev::EventType::SYNCHRONIZATION,
                evdev::Synchronization::SYN_REPORT.0,
                0,
            ),
        ])?;
        Ok(())
    }
}

#[cfg(target_os = "windows")]
pub struct InputInjector;

#[cfg(target_os = "windows")]
impl InputInjector {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self)
    }

    pub fn mouse_move(&mut self, dx: i16, dy: i16) -> anyhow::Result<()> {
        use windows::Win32::UI::Input::KeyboardAndMouse::*;

        let input = windows::Win32::UI::Input::INPUT {
            r#type: windows::Win32::UI::Input::INPUT_TYPE(0),
            Anonymous: windows::Win32::UI::Input::INPUT_0 {
                mi: windows::Win32::UI::Input::MOUSEINPUT {
                    dx: dx as i32,
                    dy: dy as i32,
                    mouseData: 0,
                    dwFlags: MOUSE_EVENT_FLAGS(0x0001), // MOUSEEVENTF_MOVE
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        unsafe {
            SendInput(&[input], std::mem::size_of::<windows::Win32::UI::Input::INPUT>() as i32);
        }
        Ok(())
    }

    pub fn mouse_button(&mut self, button: MouseButtonId, state: ButtonState) -> anyhow::Result<()> {
        use windows::Win32::UI::Input::KeyboardAndMouse::*;

        let flag = match (button, state) {
            (MouseButtonId::Left, ButtonState::Pressed) => MOUSE_EVENT_FLAGS(0x0002),   // MOUSEEVENTF_LEFTDOWN
            (MouseButtonId::Left, ButtonState::Released) => MOUSE_EVENT_FLAGS(0x0004),  // MOUSEEVENTF_LEFTUP
            (MouseButtonId::Right, ButtonState::Pressed) => MOUSE_EVENT_FLAGS(0x0008),  // MOUSEEVENTF_RIGHTDOWN
            (MouseButtonId::Right, ButtonState::Released) => MOUSE_EVENT_FLAGS(0x0010), // MOUSEEVENTF_RIGHTUP
            (MouseButtonId::Middle, ButtonState::Pressed) => MOUSE_EVENT_FLAGS(0x0020), // MOUSEEVENTF_MIDDLEDOWN
            (MouseButtonId::Middle, ButtonState::Released) => MOUSE_EVENT_FLAGS(0x0040),// MOUSEEVENTF_MIDDLEUP
            _ => return Ok(()),
        };

        let input = windows::Win32::UI::Input::INPUT {
            r#type: windows::Win32::UI::Input::INPUT_TYPE(0),
            Anonymous: windows::Win32::UI::Input::INPUT_0 {
                mi: windows::Win32::UI::Input::MOUSEINPUT {
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
            SendInput(&[input], std::mem::size_of::<windows::Win32::UI::Input::INPUT>() as i32);
        }
        Ok(())
    }

    pub fn mouse_scroll(&mut self, delta: i16) -> anyhow::Result<()> {
        use windows::Win32::UI::Input::KeyboardAndMouse::*;

        let input = windows::Win32::UI::Input::INPUT {
            r#type: windows::Win32::UI::Input::INPUT_TYPE(0),
            Anonymous: windows::Win32::UI::Input::INPUT_0 {
                mi: windows::Win32::UI::Input::MOUSEINPUT {
                    dx: 0,
                    dy: 0,
                    mouseData: (delta as i32) * 120u32 as i32,
                    dwFlags: MOUSE_EVENT_FLAGS(0x0800), // MOUSEEVENTF_WHEEL
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        unsafe {
            SendInput(&[input], std::mem::size_of::<windows::Win32::UI::Input::INPUT>() as i32);
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

        let input = windows::Win32::UI::Input::INPUT {
            r#type: windows::Win32::UI::Input::INPUT_TYPE(1),
            Anonymous: windows::Win32::UI::Input::INPUT_0 {
                ki: windows::Win32::UI::Input::KEYBDINPUT {
                    wVk: VIRTUAL_KEY(keycode),
                    wScan: 0,
                    dwFlags: flag,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        unsafe {
            SendInput(&[input], std::mem::size_of::<windows::Win32::UI::Input::INPUT>() as i32);
        }
        Ok(())
    }
}
