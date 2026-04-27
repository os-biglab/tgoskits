//! [ArceOS](https://github.com/rcore-os/arceos) network module.
//!
//! It provides unified networking primitives for TCP/UDP communication
//! using various underlying network stacks. Currently, only [smoltcp] is
//! supported.
//!
//! # Organization
//!
//! - [`tcp::TcpSocket`]: A TCP socket that provides POSIX-like APIs.
//! - [`udp::UdpSocket`]: A UDP socket that provides POSIX-like APIs.
//!
//! [smoltcp]: https://github.com/smoltcp-rs/smoltcp

#![no_std]

#[macro_use]
extern crate log;
extern crate alloc;

mod consts;
mod device;
mod general;
mod listen_table;
/// Socket option types and the [`Configurable`](options::Configurable) trait.
pub mod options;
mod router;
mod service;
mod socket;
pub(crate) mod state;
/// TCP socket implementation.
pub mod tcp;
/// UDP socket implementation.
pub mod udp;
/// Unix domain socket implementation.
pub mod unix;
/// Vsock socket implementation.
#[cfg(feature = "vsock")]
pub mod vsock;
mod wrapper;

use alloc::{borrow::ToOwned, boxed::Box};

use ax_driver::{AxDeviceContainer, prelude::*};
use ax_sync::Mutex;
use smoltcp::wire::{EthernetAddress, Ipv4Address, Ipv4Cidr, Ipv6Address, Ipv6Cidr};
use spin::{Lazy, Once};

pub use self::socket::*;
use self::{
    consts::{GATEWAY, GW6, IP, IP_PREFIX, IP6},
    device::{EthernetDevice, LoopbackDevice},
    listen_table::ListenTable,
    router::{Router, Rule},
    service::Service,
    wrapper::SocketSetWrapper,
};

static LISTEN_TABLE: Lazy<ListenTable> = Lazy::new(ListenTable::new);
static SOCKET_SET: Lazy<SocketSetWrapper> = Lazy::new(SocketSetWrapper::new);

static SERVICE: Once<Mutex<Service>> = Once::new();

fn get_service() -> ax_sync::MutexGuard<'static, Service> {
    SERVICE
        .get()
        .expect("Network service not initialized")
        .lock()
}

/// Initializes the network subsystem by NIC devices.
pub fn init_network(mut net_devs: AxDeviceContainer<AxNetDevice>) {
    info!("Initialize network subsystem...");

    let mut router = Router::new();
    let lo_dev = router.add_device(Box::new(LoopbackDevice::new()));

    let lo_ip = Ipv4Cidr::new(Ipv4Address::new(127, 0, 0, 1), 8);
    router.add_rule(Rule::new(
        lo_ip.into(),
        None,
        lo_dev,
        lo_ip.address().into(),
    ));

    let lo_ip6 = Ipv6Cidr::new(Ipv6Address::new(0, 0, 0, 0, 0, 0, 0, 1), 128);
    router.add_rule(Rule::new(
        lo_ip6.into(),
        None,
        lo_dev,
        lo_ip6.address().into(),
    ));

    let (eth0_ip, eth0_ip6) = if let Some(dev) = net_devs.take_one() {
        info!("  use NIC 0: {:?}", dev.device_name());

        let eth0_address = EthernetAddress(dev.mac_address().0);
        let eth0_ip = Ipv4Cidr::new(IP.parse().expect("Invalid IPv4 address"), IP_PREFIX);

        let eth0_dev = router.add_device(Box::new(EthernetDevice::new(
            "eth0".to_owned(),
            dev,
            eth0_ip,
            (!IP6.is_empty()).then(|| IP6.parse().expect("Invalid IPv6 address (AX_IP6)")),
        )));

        router.add_rule(Rule::new(
            Ipv4Cidr::new(Ipv4Address::UNSPECIFIED, 0).into(),
            Some(GATEWAY.parse().expect("Invalid gateway address")),
            eth0_dev,
            eth0_ip.address().into(),
        ));

        info!("eth0:");
        info!("  mac:  {}", eth0_address);
        info!("  ip:   {}", eth0_ip);

        // Configure IPv6 on eth0 when AX_IP6/AX_GW6 are set (e.g. QEMU SLIRP fec0::/64).
        let eth0_ip6 = if !IP6.is_empty() && !GW6.is_empty() {
            let ip6: Ipv6Address = IP6.parse().expect("Invalid IPv6 address (AX_IP6)");
            let gw6: Ipv6Address = GW6.parse().expect("Invalid IPv6 gateway (AX_GW6)");
            let cidr6 = Ipv6Cidr::new(ip6, 64);

            router.add_rule(Rule::new(
                Ipv6Cidr::new(Ipv6Address::UNSPECIFIED, 0).into(),
                Some(gw6.into()),
                eth0_dev,
                ip6.into(),
            ));

            info!("  ip6:  {}", cidr6);
            Some(cidr6)
        } else {
            None
        };

        (Some(eth0_ip), eth0_ip6)
    } else {
        warn!("  No network device found!");
        (None, None)
    };

    for dev in &router.devices {
        info!("Device: {}", dev.name());
    }

    let mut service = Service::new(router);
    service.iface.update_ip_addrs(|ip_addrs| {
        ip_addrs.push(lo_ip.into()).unwrap();
        ip_addrs.push(lo_ip6.into()).unwrap();
        if let Some(eth0_ip) = eth0_ip {
            ip_addrs.push(eth0_ip.into()).unwrap();
        }
        if let Some(eth0_ip6) = eth0_ip6 {
            ip_addrs.push(eth0_ip6.into()).unwrap();
        }
    });
    SERVICE.call_once(|| Mutex::new(service));
}

/// Init vsock subsystem by vsock devices.
#[cfg(feature = "vsock")]
pub fn init_vsock(mut vsock_devs: AxDeviceContainer<AxVsockDevice>) {
    use self::device::register_vsock_device;
    info!("Initialize vsock subsystem...");
    if let Some(dev) = vsock_devs.take_one() {
        info!("  use vsock 0: {:?}", dev.device_name());
        if let Err(e) = register_vsock_device(dev) {
            warn!("Failed to initialize vsock device: {:?}", e);
        }
    } else {
        warn!("  No vsock device found!");
    }
}

/// Poll all network interfaces for new events.
pub fn poll_interfaces() {
    while get_service().poll(&mut SOCKET_SET.inner.lock()) {}
}
