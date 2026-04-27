macro_rules! env_or_default {
    ($key:literal) => {
        match option_env!($key) {
            Some(val) => val,
            None => "",
        }
    };
}

pub const IP: &str = env_or_default!("AX_IP");
pub const GATEWAY: &str = env_or_default!("AX_GW");
pub const IP_PREFIX: u8 = 24;

/// IPv6 address for eth0 (QEMU SLIRP default prefix `fec0::/64`).
/// Leave empty to disable IPv6 on eth0.
pub const IP6: &str = env_or_default!("AX_IP6");
/// IPv6 default gateway (QEMU SLIRP host address).
pub const GW6: &str = env_or_default!("AX_GW6");

pub const STANDARD_MTU: usize = 1500;

pub const TCP_RX_BUF_LEN: usize = 64 * 1024;
pub const TCP_TX_BUF_LEN: usize = 64 * 1024;
pub const UDP_RX_BUF_LEN: usize = 64 * 1024;
pub const UDP_TX_BUF_LEN: usize = 64 * 1024;
pub const LISTEN_QUEUE_SIZE: usize = 512;

pub const SOCKET_BUFFER_SIZE: usize = 64;
pub const ETHERNET_MAX_PENDING_PACKETS: usize = 32;
