use std::fmt;

#[derive(Debug)]
pub enum TransportProtocol {
    UDP,
    TCP,
    ANY,
}

impl fmt::Display for TransportProtocol {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            TransportProtocol::UDP => write!(f, "udp"),
            TransportProtocol::TCP => write!(f, "tcp"),
            TransportProtocol::ANY => write!(f, "-1"),
        }
    }
}
