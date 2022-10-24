/// Enum to specify how a packet should be delivered.
#[derive(Copy, Clone, Debug, PartialOrd, PartialEq, Eq)]
pub enum DeliveryGuarantee {
    /// Packet may or may not be delivered
    Unreliable,
    /// Packet will be delivered
    Reliable,
}

/// Enum to specify how a packet should be arranged.
#[derive(Copy, Clone, Debug, PartialOrd, PartialEq, Eq)]
pub enum OrderingGuarantee {
    /// No arranging will be done.
    None,
    /// Packets will be arranged in sequence.
    Sequenced(Option<u8>),
    /// Packets will be arranged in order.
    Ordered(Option<u8>),
}

impl Default for OrderingGuarantee {
    fn default() -> Self {
        OrderingGuarantee::None
    }
}