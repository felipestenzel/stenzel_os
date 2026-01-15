//! Drivers de rede.

pub mod virtio_net;

pub use virtio_net::{init, get_mac, send, recv};
