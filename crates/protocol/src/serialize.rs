use bytes::{Buf, BufMut, BytesMut};
use thiserror::Error;

use crate::{
    message::*, MAGIC,
};

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("invalid magic: expected 0x{expected:04X}, got 0x{got:04X}")]
    InvalidMagic { expected: u16, got: u16 },
    #[error("unknown message type: 0x{0:02X}")]
    UnknownMessageType(u8),
    #[error("buffer too short: need {needed}, have {available}")]
    BufferTooShort { needed: usize, available: usize },
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

const HEADER_SIZE: usize = 4;

pub fn encode(msg: &Message, buf: &mut BytesMut) {
    let payload = encode_payload(msg);
    buf.reserve(HEADER_SIZE + payload.len());
    buf.put_u16_le(MAGIC);
    buf.put_u8(msg.msg_type() as u8);
    buf.put_u8(0); // reserved
    buf.extend_from_slice(&payload);
}

pub fn decode(buf: &mut BytesMut) -> Result<Option<Message>, ProtocolError> {
    if buf.len() < HEADER_SIZE {
        return Ok(None);
    }

    let magic = buf.chunk()[0..2].try_into().map(u16::from_le_bytes).unwrap();
    if magic != MAGIC {
        buf.advance(1);
        return Err(ProtocolError::InvalidMagic {
            expected: MAGIC,
            got: magic,
        });
    }

    let msg_type_byte = buf.chunk()[2];
    let msg_type = MessageType::try_from(msg_type_byte)
        .map_err(|_| ProtocolError::UnknownMessageType(msg_type_byte))?;

    let payload = match msg_type {
        MessageType::Hello
        | MessageType::HelloAck => 4 + 1,
        MessageType::MouseMove => 8,
        MessageType::MouseButton => 2,
        MessageType::MouseScroll => 2,
        MessageType::KeyDown | MessageType::KeyUp => 2,
        MessageType::Clipboard => 4,
        MessageType::EdgeEnter => 3,
        MessageType::EdgeLeave => 1,
        MessageType::ScreenInfo => 4,
        MessageType::Heartbeat => 0,
    };

    let total = HEADER_SIZE + payload;

    let clipboard_min = HEADER_SIZE + 4;
    if msg_type == MessageType::Clipboard {
        if buf.len() < clipboard_min {
            return Ok(None);
        }
        let clip_len =
            u32::from_le_bytes(buf.chunk()[HEADER_SIZE..HEADER_SIZE + 4].try_into().unwrap())
                as usize;
        if buf.len() < HEADER_SIZE + 4 + clip_len {
            return Ok(None);
        }
    } else if buf.len() < total {
        return Ok(None);
    }

    buf.advance(HEADER_SIZE);

    let msg = decode_message(msg_type, buf)?;

    Ok(Some(msg))
}

fn encode_payload(msg: &Message) -> Vec<u8> {
    let mut buf = Vec::new();
    match msg {
        Message::Hello(p) => {
            buf.put_u16_le(p.screen.width);
            buf.put_u16_le(p.screen.height);
            buf.put_u8(match p.screen.width > 0 { true => 0, false => 1 });
        }
        Message::HelloAck(p) => {
            buf.put_u16_le(p.screen.width);
            buf.put_u16_le(p.screen.height);
            buf.put_u8(layout_to_byte(&p.layout));
        }
        Message::MouseMove(p) => {
            buf.put_i32_le(p.x);
            buf.put_i32_le(p.y);
        }
        Message::MouseButton(p) => {
            buf.put_u8(p.button as u8);
            buf.put_u8(p.state as u8);
        }
        Message::MouseScroll(p) => {
            buf.put_i16_le(p.delta);
        }
        Message::KeyDown(p) | Message::KeyUp(p) => {
            buf.put_u16_le(p.keycode);
        }
        Message::Clipboard(p) => {
            buf.put_u32_le(p.data.len() as u32);
            buf.extend_from_slice(&p.data);
        }
        Message::EdgeEnter(p) => {
            buf.put_u8(p.edge as u8);
            buf.put_u16_le(p.position);
        }
        Message::EdgeLeave(p) => {
            buf.put_u8(p.edge as u8);
        }
        Message::ScreenInfo(p) => {
            buf.put_u16_le(p.width);
            buf.put_u16_le(p.height);
        }
        Message::Heartbeat => {}
    }
    buf
}

fn decode_message(msg_type: MessageType, buf: &mut BytesMut) -> Result<Message, ProtocolError> {
    let msg = match msg_type {
        MessageType::Hello => {
            let width = buf.get_u16_le();
            let height = buf.get_u16_le();
            let _layout = buf.get_u8();
            Message::Hello(HelloPayload {
                screen: ScreenInfo { width, height },
            })
        }
        MessageType::HelloAck => {
            let width = buf.get_u16_le();
            let height = buf.get_u16_le();
            let layout_byte = buf.get_u8();
            Message::HelloAck(HelloAckPayload {
                screen: ScreenInfo { width, height },
                layout: byte_to_layout(layout_byte),
            })
        }
        MessageType::MouseMove => {
            let x = buf.get_i32_le();
            let y = buf.get_i32_le();
            Message::MouseMove(MouseMovePayload { x, y })
        }
        MessageType::MouseButton => {
            let button = byte_to_button(buf.get_u8());
            let state = byte_to_state(buf.get_u8());
            Message::MouseButton(MouseButtonPayload { button, state })
        }
        MessageType::MouseScroll => {
            let delta = buf.get_i16_le();
            Message::MouseScroll(MouseScrollPayload { delta })
        }
        MessageType::KeyDown => {
            let keycode = buf.get_u16_le();
            Message::KeyDown(KeyPayload { keycode })
        }
        MessageType::KeyUp => {
            let keycode = buf.get_u16_le();
            Message::KeyUp(KeyPayload { keycode })
        }
        MessageType::Clipboard => {
            let len = buf.get_u32_le() as usize;
            let data = buf.split_to(len).to_vec();
            Message::Clipboard(ClipboardPayload { data })
        }
        MessageType::EdgeEnter => {
            let edge = byte_to_edge(buf.get_u8());
            let position = buf.get_u16_le();
            Message::EdgeEnter(EdgeEnterPayload { edge, position })
        }
        MessageType::EdgeLeave => {
            let edge = byte_to_edge(buf.get_u8());
            Message::EdgeLeave(EdgeLeavePayload { edge })
        }
        MessageType::ScreenInfo => {
            let width = buf.get_u16_le();
            let height = buf.get_u16_le();
            Message::ScreenInfo(ScreenInfo { width, height })
        }
        MessageType::Heartbeat => Message::Heartbeat,
    };
    Ok(msg)
}

fn layout_to_byte(layout: &LayoutPosition) -> u8 {
    match layout {
        LayoutPosition::LeftRight => 0,
        LayoutPosition::RightLeft => 1,
        LayoutPosition::TopBottom => 2,
        LayoutPosition::BottomTop => 3,
    }
}

fn byte_to_layout(b: u8) -> LayoutPosition {
    match b {
        1 => LayoutPosition::RightLeft,
        2 => LayoutPosition::TopBottom,
        3 => LayoutPosition::BottomTop,
        _ => LayoutPosition::LeftRight,
    }
}

fn byte_to_button(b: u8) -> MouseButtonId {
    match b {
        1 => MouseButtonId::Middle,
        2 => MouseButtonId::Right,
        3 => MouseButtonId::Side1,
        4 => MouseButtonId::Side2,
        _ => MouseButtonId::Left,
    }
}

fn byte_to_state(b: u8) -> ButtonState {
    match b {
        1 => ButtonState::Pressed,
        _ => ButtonState::Released,
    }
}

fn byte_to_edge(b: u8) -> Edge {
    match b {
        1 => Edge::Right,
        2 => Edge::Top,
        3 => Edge::Bottom,
        _ => Edge::Left,
    }
}
