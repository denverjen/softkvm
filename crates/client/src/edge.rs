use softkvm_protocol::message::Edge;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusTarget {
    Server,
    Client,
}

pub struct EdgeDetector {
    pub target: FocusTarget,
    pub client_width: i32,
    pub client_height: i32,
    pub edge_size: i32,
}

impl EdgeDetector {
    pub fn new(client_width: i32, client_height: i32) -> Self {
        Self {
            target: FocusTarget::Client,
            client_width,
            client_height,
            edge_size: 2,
        }
    }

    pub fn check(&mut self, x: i32, y: i32, layout: &softkvm_protocol::message::LayoutPosition) -> Option<Edge> {
        use softkvm_protocol::message::LayoutPosition;
        match self.target {
            FocusTarget::Client => match layout {
                LayoutPosition::LeftRight => {
                    if x <= self.edge_size {
                        self.target = FocusTarget::Server;
                        return Some(Edge::Left);
                    }
                }
                LayoutPosition::RightLeft => {
                    if x >= self.client_width - self.edge_size {
                        self.target = FocusTarget::Server;
                        return Some(Edge::Right);
                    }
                }
                LayoutPosition::TopBottom => {
                    if y <= self.edge_size {
                        self.target = FocusTarget::Server;
                        return Some(Edge::Top);
                    }
                }
                LayoutPosition::BottomTop => {
                    if y >= self.client_height - self.edge_size {
                        self.target = FocusTarget::Server;
                        return Some(Edge::Bottom);
                    }
                }
            },
            FocusTarget::Server => {}
        }
        None
    }

    pub fn return_to_client(&mut self) {
        self.target = FocusTarget::Client;
    }

    pub fn is_server_focused(&self) -> bool {
        self.target == FocusTarget::Server
    }
}
