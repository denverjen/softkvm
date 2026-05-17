use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MessageType {
    Hello = 0x01,
    HelloAck = 0x02,
    MouseMove = 0x10,
    MouseButton = 0x11,
    MouseScroll = 0x12,
    KeyDown = 0x20,
    KeyUp = 0x21,
    Clipboard = 0x30,
    EdgeEnter = 0x40,
    EdgeLeave = 0x41,
    ScreenInfo = 0x50,
    Heartbeat = 0xFF,
}

impl TryFrom<u8> for MessageType {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x01 => Ok(Self::Hello),
            0x02 => Ok(Self::HelloAck),
            0x10 => Ok(Self::MouseMove),
            0x11 => Ok(Self::MouseButton),
            0x12 => Ok(Self::MouseScroll),
            0x20 => Ok(Self::KeyDown),
            0x21 => Ok(Self::KeyUp),
            0x30 => Ok(Self::Clipboard),
            0x40 => Ok(Self::EdgeEnter),
            0x41 => Ok(Self::EdgeLeave),
            0x50 => Ok(Self::ScreenInfo),
            0xFF => Ok(Self::Heartbeat),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MouseButtonId {
    Left = 0,
    Middle = 1,
    Right = 2,
    Side1 = 3,
    Side2 = 4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ButtonState {
    Released = 0,
    Pressed = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Edge {
    Left = 0,
    Right = 1,
    Top = 2,
    Bottom = 3,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScreenInfo {
    pub width: u16,
    pub height: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelloPayload {
    pub screen: ScreenInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelloAckPayload {
    pub screen: ScreenInfo,
    pub layout: LayoutPosition,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LayoutPosition {
    LeftRight,
    RightLeft,
    TopBottom,
    BottomTop,
}

#[derive(Debug, Clone)]
pub struct MouseMovePayload {
    pub dx: i16,
    pub dy: i16,
}

#[derive(Debug, Clone)]
pub struct MouseButtonPayload {
    pub button: MouseButtonId,
    pub state: ButtonState,
}

#[derive(Debug, Clone)]
pub struct MouseScrollPayload {
    pub delta: i16,
}

#[derive(Debug, Clone)]
pub struct KeyPayload {
    pub keycode: u16,
}

#[derive(Debug, Clone)]
pub struct ClipboardPayload {
    pub data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct EdgeEnterPayload {
    pub edge: Edge,
    pub position: u16,
}

#[derive(Debug, Clone)]
pub struct EdgeLeavePayload {
    pub edge: Edge,
}

#[derive(Debug, Clone)]
pub enum Message {
    Hello(HelloPayload),
    HelloAck(HelloAckPayload),
    MouseMove(MouseMovePayload),
    MouseButton(MouseButtonPayload),
    MouseScroll(MouseScrollPayload),
    KeyDown(KeyPayload),
    KeyUp(KeyPayload),
    Clipboard(ClipboardPayload),
    EdgeEnter(EdgeEnterPayload),
    EdgeLeave(EdgeLeavePayload),
    ScreenInfo(ScreenInfo),
    Heartbeat,
}

impl Message {
    pub fn msg_type(&self) -> MessageType {
        match self {
            Self::Hello(_) => MessageType::Hello,
            Self::HelloAck(_) => MessageType::HelloAck,
            Self::MouseMove(_) => MessageType::MouseMove,
            Self::MouseButton(_) => MessageType::MouseButton,
            Self::MouseScroll(_) => MessageType::MouseScroll,
            Self::KeyDown(_) => MessageType::KeyDown,
            Self::KeyUp(_) => MessageType::KeyUp,
            Self::Clipboard(_) => MessageType::Clipboard,
            Self::EdgeEnter(_) => MessageType::EdgeEnter,
            Self::EdgeLeave(_) => MessageType::EdgeLeave,
            Self::ScreenInfo(_) => MessageType::ScreenInfo,
            Self::Heartbeat => MessageType::Heartbeat,
        }
    }
}
