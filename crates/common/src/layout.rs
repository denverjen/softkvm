use softkvm_protocol::message::LayoutPosition;

pub struct ScreenLayout {
    pub server_width: u16,
    pub server_height: u16,
    pub client_width: u16,
    pub client_height: u16,
    pub position: LayoutPosition,
}

impl ScreenLayout {
    pub fn new(
        server_w: u16,
        server_h: u16,
        client_w: u16,
        client_h: u16,
        position: LayoutPosition,
    ) -> Self {
        Self {
            server_width: server_w,
            server_height: server_h,
            client_width: client_w,
            client_height: client_h,
            position,
        }
    }

    pub fn is_at_server_edge(&self, x: i32, y: i32) -> Option<softkvm_protocol::message::Edge> {
        use softkvm_protocol::message::Edge;
        match self.position {
            LayoutPosition::LeftRight => {
                if x >= self.server_width as i32 - 1 {
                    return Some(Edge::Right);
                }
            }
            LayoutPosition::RightLeft => {
                if x <= 0 {
                    return Some(Edge::Left);
                }
            }
            LayoutPosition::TopBottom => {
                if y >= self.server_height as i32 - 1 {
                    return Some(Edge::Bottom);
                }
            }
            LayoutPosition::BottomTop => {
                if y <= 0 {
                    return Some(Edge::Top);
                }
            }
        }
        None
    }

    pub fn is_at_client_edge(&self, x: i32, y: i32) -> Option<softkvm_protocol::message::Edge> {
        use softkvm_protocol::message::Edge;
        match self.position {
            LayoutPosition::LeftRight => {
                if x <= 0 {
                    return Some(Edge::Left);
                }
            }
            LayoutPosition::RightLeft => {
                if x >= self.client_width as i32 - 1 {
                    return Some(Edge::Right);
                }
            }
            LayoutPosition::TopBottom => {
                if y <= 0 {
                    return Some(Edge::Top);
                }
            }
            LayoutPosition::BottomTop => {
                if y >= self.client_height as i32 - 1 {
                    return Some(Edge::Bottom);
                }
            }
        }
        None
    }
}
