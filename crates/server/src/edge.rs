use softkvm_protocol::message::Edge;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusTarget {
    Server,
    Client,
}

pub struct EdgeDetector {
    pub target: FocusTarget,
    pub server_width: i32,
    pub server_height: i32,
    pub edge_size: i32,
}

impl EdgeDetector {
    pub fn new(server_width: i32, server_height: i32) -> Self {
        Self {
            target: FocusTarget::Server,
            server_width,
            server_height,
            edge_size: 2,
        }
    }

    pub fn check(&mut self, x: i32, y: i32, layout: &softkvm_protocol::message::LayoutPosition) -> Option<Edge> {
        use softkvm_protocol::message::LayoutPosition;
        match self.target {
            FocusTarget::Server => match layout {
                LayoutPosition::LeftRight => {
                    if x >= self.server_width - self.edge_size {
                        self.target = FocusTarget::Client;
                        return Some(Edge::Right);
                    }
                }
                LayoutPosition::RightLeft => {
                    if x <= self.edge_size {
                        self.target = FocusTarget::Client;
                        return Some(Edge::Left);
                    }
                }
                LayoutPosition::TopBottom => {
                    if y >= self.server_height - self.edge_size {
                        self.target = FocusTarget::Client;
                        return Some(Edge::Bottom);
                    }
                }
                LayoutPosition::BottomTop => {
                    if y <= self.edge_size {
                        self.target = FocusTarget::Client;
                        return Some(Edge::Top);
                    }
                }
            },
            FocusTarget::Client => {}
        }
        None
    }

    pub fn return_to_server(&mut self) {
        self.target = FocusTarget::Server;
    }

    pub fn is_client_focused(&self) -> bool {
        self.target == FocusTarget::Client
    }
}
