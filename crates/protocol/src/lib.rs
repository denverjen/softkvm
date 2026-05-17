pub mod message;
pub mod serialize;

pub use message::Message;

pub const MAGIC: u16 = 0x5F4B;
pub const DEFAULT_PORT: u16 = 24800;
