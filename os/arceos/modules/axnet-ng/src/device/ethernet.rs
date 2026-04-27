use alloc::{string::String, vec};
use core::task::Waker;

use ax_driver::prelude::*;
use ax_task::future::register_irq_waker;
use hashbrown::HashMap;
use smoltcp::{
    phy::ChecksumCapabilities,
    storage::{PacketBuffer, PacketMetadata},
    time::{Duration, Instant},
    wire::{
        ArpOperation, ArpPacket, ArpRepr, EthernetAddress, EthernetFrame, EthernetProtocol,
        EthernetRepr, Icmpv6Packet, Icmpv6Repr, IpAddress, IpProtocol, Ipv4Cidr, Ipv6Address,
        Ipv6Packet, Ipv6Repr, NdiscNeighborFlags, NdiscRepr,
    },
};

use crate::{
    consts::{ETHERNET_MAX_PENDING_PACKETS, STANDARD_MTU},
    device::Device,
};

const EMPTY_MAC: EthernetAddress = EthernetAddress([0; 6]);

struct Neighbor {
    hardware_address: EthernetAddress,
    expires_at: Instant,
}

pub struct EthernetDevice {
    name: String,
    inner: AxNetDevice,
    neighbors: HashMap<IpAddress, Option<Neighbor>>,
    ip: Ipv4Cidr,
    ip6: Option<Ipv6Address>,

    pending_packets: PacketBuffer<'static, IpAddress>,
}
impl EthernetDevice {
    const NEIGHBOR_TTL: Duration = Duration::from_secs(60);

    pub fn new(name: String, inner: AxNetDevice, ip: Ipv4Cidr, ip6: Option<Ipv6Address>) -> Self {
        let pending_packets = PacketBuffer::new(
            vec![PacketMetadata::EMPTY; ETHERNET_MAX_PENDING_PACKETS],
            vec![
                0u8;
                (STANDARD_MTU + EthernetFrame::<&[u8]>::header_len())
                    * ETHERNET_MAX_PENDING_PACKETS
            ],
        );
        Self {
            name,
            inner,
            neighbors: HashMap::new(),
            ip,
            ip6,

            pending_packets,
        }
    }

    #[inline]
    fn hardware_address(&self) -> EthernetAddress {
        EthernetAddress(self.inner.mac_address().0)
    }

    fn send_to<F>(
        inner: &mut AxNetDevice,
        dst: EthernetAddress,
        size: usize,
        f: F,
        proto: EthernetProtocol,
    ) where
        F: FnOnce(&mut [u8]),
    {
        if let Err(err) = inner.recycle_tx_buffers() {
            warn!("recycle_tx_buffers failed: {:?}", err);
            return;
        }

        let repr = EthernetRepr {
            src_addr: EthernetAddress(inner.mac_address().0),
            dst_addr: dst,
            ethertype: proto,
        };

        let mut tx_buf = match inner.alloc_tx_buffer(repr.buffer_len() + size) {
            Ok(buf) => buf,
            Err(err) => {
                warn!("alloc_tx_buffer failed: {:?}", err);
                return;
            }
        };
        let mut frame = EthernetFrame::new_unchecked(tx_buf.packet_mut());
        repr.emit(&mut frame);
        f(frame.payload_mut());
        trace!(
            "SEND {} bytes: {:02X?}",
            tx_buf.packet_len(),
            tx_buf.packet()
        );
        if let Err(err) = inner.transmit(tx_buf) {
            warn!("transmit failed: {:?}", err);
        }
    }

    fn handle_frame(
        &mut self,
        frame: &[u8],
        buffer: &mut PacketBuffer<()>,
        timestamp: Instant,
    ) -> bool {
        let frame = EthernetFrame::new_unchecked(frame);
        let Ok(repr) = EthernetRepr::parse(&frame) else {
            warn!("Dropping malformed Ethernet frame");
            return false;
        };

        if !repr.dst_addr.is_broadcast()
            && !repr.dst_addr.is_multicast()
            && repr.dst_addr != EMPTY_MAC
            && repr.dst_addr != self.hardware_address()
        {
            info!(
                "{}: drop frame by dst MAC filter, dst={}, self={}, ethertype={:?}",
                self.name,
                repr.dst_addr,
                self.hardware_address(),
                repr.ethertype
            );
            return false;
        }

        match repr.ethertype {
            EthernetProtocol::Ipv4 | EthernetProtocol::Ipv6 => {
                if repr.ethertype == EthernetProtocol::Ipv6 {
                    if let Ok(ipv6) = Ipv6Packet::new_checked(frame.payload()) {
                        info!(
                            "{}: recv IPv6 frame {} -> {}, len={}",
                            self.name,
                            ipv6.src_addr(),
                            ipv6.dst_addr(),
                            frame.payload().len()
                        );
                    } else {
                        info!(
                            "{}: recv malformed IPv6 payload, len={}",
                            self.name,
                            frame.payload().len()
                        );
                    }
                    self.process_ipv6_control(frame.payload(), timestamp);
                }
                buffer
                    .enqueue(frame.payload().len(), ())
                    .unwrap()
                    .copy_from_slice(frame.payload());
                return true;
            }
            EthernetProtocol::Arp => self.process_arp(frame.payload(), timestamp),
            _ => {}
        }

        false
    }

    fn request_arp(&mut self, target_ip: IpAddress) {
        let IpAddress::Ipv4(target_ipv4) = target_ip else {
            warn!("IPv6 address ARP is not supported: {}", target_ip);
            return;
        };
        debug!("Requesting ARP for {}", target_ipv4);

        let arp_repr = ArpRepr::EthernetIpv4 {
            operation: ArpOperation::Request,
            source_hardware_addr: self.hardware_address(),
            source_protocol_addr: self.ip.address(),
            target_hardware_addr: EthernetAddress::BROADCAST,
            target_protocol_addr: target_ipv4,
        };

        Self::send_to(
            &mut self.inner,
            EthernetAddress::BROADCAST,
            arp_repr.buffer_len(),
            |buf| arp_repr.emit(&mut ArpPacket::new_unchecked(buf)),
            EthernetProtocol::Arp,
        );

        self.neighbors.insert(target_ip, None);
    }

    fn ipv6_solicited_node(target: Ipv6Address) -> Ipv6Address {
        let o = target.octets();
        Ipv6Address::from([
            0xff, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0xff, o[13],
            o[14], o[15],
        ])
    }

    fn ipv6_multicast_to_ethernet(dst: Ipv6Address) -> EthernetAddress {
        let o = dst.octets();
        EthernetAddress([0x33, 0x33, o[12], o[13], o[14], o[15]])
    }

    fn request_ndp(&mut self, target_ip: Ipv6Address) {
        let Some(src_ip) = self.ip6 else {
            warn!("IPv6 NDP request skipped: local IPv6 is not configured");
            return;
        };

        let solicit = Icmpv6Repr::Ndisc(NdiscRepr::NeighborSolicit {
            target_addr: target_ip,
            lladdr: Some(self.hardware_address().into()),
        });

        let dst_ip = Self::ipv6_solicited_node(target_ip);
        let dst_mac = Self::ipv6_multicast_to_ethernet(dst_ip);
        let ip_repr = Ipv6Repr {
            src_addr: src_ip,
            dst_addr: dst_ip,
            next_header: IpProtocol::Icmpv6,
            payload_len: solicit.buffer_len(),
            hop_limit: 0xff,
        };

        Self::send_to(
            &mut self.inner,
            dst_mac,
            ip_repr.buffer_len() + solicit.buffer_len(),
            |buf| {
                let mut ip_packet = Ipv6Packet::new_unchecked(buf);
                ip_repr.emit(&mut ip_packet);
                let mut icmp_packet = Icmpv6Packet::new_unchecked(ip_packet.payload_mut());
                solicit.emit(
                    &src_ip,
                    &dst_ip,
                    &mut icmp_packet,
                    &ChecksumCapabilities::default(),
                );
            },
            EthernetProtocol::Ipv6,
        );

        debug!("NDP: sent NS for {}", target_ip);
        self.neighbors.insert(IpAddress::Ipv6(target_ip), None);
    }

    fn flush_pending_for(&mut self, next_hop: IpAddress, now: Instant, proto: EthernetProtocol) {
        if self
            .pending_packets
            .peek()
            .is_ok_and(|it| it.0 == &next_hop)
        {
            while let Ok((&pending_hop, buf)) = self.pending_packets.peek() {
                let Some(Some(neighbor)) = self.neighbors.get(&pending_hop) else {
                    break;
                };
                if neighbor.expires_at <= now {
                    match pending_hop {
                        IpAddress::Ipv4(_) => self.request_arp(pending_hop),
                        IpAddress::Ipv6(addr) => self.request_ndp(addr),
                    }
                    break;
                }

                Self::send_to(
                    &mut self.inner,
                    neighbor.hardware_address,
                    buf.len(),
                    |b| b.copy_from_slice(buf),
                    proto,
                );
                let _ = self.pending_packets.dequeue();
            }
        }
    }

    fn process_ndp_neighbor_discovery(
        &mut self,
        ipv6_src: Ipv6Address,
        ndp: NdiscRepr,
        now: Instant,
    ) {
        match ndp {
            NdiscRepr::NeighborSolicit {
                target_addr,
                lladdr,
            } => {
                if let Some(lladdr) = lladdr {
                    let ll = lladdr.as_bytes();
                    if ll.len() == 6 {
                        self.neighbors.insert(
                            IpAddress::Ipv6(ipv6_src),
                            Some(Neighbor {
                                hardware_address: EthernetAddress::from_bytes(ll),
                                expires_at: now + Self::NEIGHBOR_TTL,
                            }),
                        );
                    }
                }

                if Some(target_addr) != self.ip6 {
                    return;
                }

                let Some(src_ip) = self.ip6 else {
                    return;
                };

                let Some(lladdr) = lladdr else {
                    debug!(
                        "NDP: NS for {} has no source lladdr, skip NA response",
                        target_addr
                    );
                    return;
                };
                let ll = lladdr.as_bytes();
                if ll.len() != 6 {
                    return;
                }
                let dst_mac = EthernetAddress::from_bytes(ll);

                let advert = Icmpv6Repr::Ndisc(NdiscRepr::NeighborAdvert {
                    flags: NdiscNeighborFlags::SOLICITED | NdiscNeighborFlags::OVERRIDE,
                    target_addr,
                    lladdr: Some(self.hardware_address().into()),
                });
                let ip_repr = Ipv6Repr {
                    src_addr: src_ip,
                    dst_addr: ipv6_src,
                    next_header: IpProtocol::Icmpv6,
                    payload_len: advert.buffer_len(),
                    hop_limit: 0xff,
                };

                Self::send_to(
                    &mut self.inner,
                    dst_mac,
                    ip_repr.buffer_len() + advert.buffer_len(),
                    |buf| {
                        let mut ip_packet = Ipv6Packet::new_unchecked(buf);
                        ip_repr.emit(&mut ip_packet);
                        let mut icmp_packet = Icmpv6Packet::new_unchecked(ip_packet.payload_mut());
                        advert.emit(
                            &src_ip,
                            &ipv6_src,
                            &mut icmp_packet,
                            &ChecksumCapabilities::default(),
                        );
                    },
                    EthernetProtocol::Ipv6,
                );
                info!("NDP: replied NA for {} to {}", target_addr, ipv6_src);
            }
            NdiscRepr::NeighborAdvert {
                target_addr,
                lladdr,
                ..
            } => {
                let Some(lladdr) = lladdr else {
                    return;
                };
                let ll = lladdr.as_bytes();
                if ll.len() != 6 {
                    return;
                }
                let hw = EthernetAddress::from_bytes(ll);
                self.neighbors.insert(
                    IpAddress::Ipv6(target_addr),
                    Some(Neighbor {
                        hardware_address: hw,
                        expires_at: now + Self::NEIGHBOR_TTL,
                    }),
                );
                debug!("NDP: learned {} -> {}", target_addr, hw);
                self.flush_pending_for(IpAddress::Ipv6(target_addr), now, EthernetProtocol::Ipv6);
            }
            _ => {}
        }
    }

    fn process_ipv6_control(&mut self, payload: &[u8], now: Instant) {
        let Ok(ipv6) = Ipv6Packet::new_checked(payload) else {
            return;
        };
        if ipv6.next_header() != IpProtocol::Icmpv6 {
            return;
        }
        let Ok(icmp_packet) = Icmpv6Packet::new_checked(ipv6.payload()) else {
            return;
        };
        if !icmp_packet.msg_type().is_ndisc() || icmp_packet.msg_code() != 0 {
            return;
        }
        if let Ok(ndp) = NdiscRepr::parse(&icmp_packet) {
            self.process_ndp_neighbor_discovery(ipv6.src_addr(), ndp, now);
        }
    }

    fn process_arp(&mut self, payload: &[u8], now: Instant) {
        let Ok(repr) = ArpPacket::new_checked(payload).and_then(|packet| ArpRepr::parse(&packet))
        else {
            warn!("Dropping malformed ARP packet");
            return;
        };

        if let ArpRepr::EthernetIpv4 {
            operation,
            source_hardware_addr,
            source_protocol_addr,
            target_hardware_addr,
            target_protocol_addr,
        } = repr
        {
            let is_unicast_mac =
                target_hardware_addr != EMPTY_MAC && !target_hardware_addr.is_broadcast();
            if is_unicast_mac && self.hardware_address() != target_hardware_addr {
                // Only process packet that are for us
                return;
            }

            if let ArpOperation::Unknown(_) = operation {
                return;
            }

            if !source_hardware_addr.is_unicast()
                || source_protocol_addr.is_broadcast()
                || source_protocol_addr.is_multicast()
                || source_protocol_addr.is_unspecified()
            {
                return;
            }
            if self.ip.address() != target_protocol_addr {
                return;
            }

            debug!("ARP: {} -> {}", source_protocol_addr, source_hardware_addr);
            self.neighbors.insert(
                IpAddress::Ipv4(source_protocol_addr),
                Some(Neighbor {
                    hardware_address: source_hardware_addr,
                    expires_at: now + Self::NEIGHBOR_TTL,
                }),
            );

            if let ArpOperation::Request = operation {
                let response = ArpRepr::EthernetIpv4 {
                    operation: ArpOperation::Reply,
                    source_hardware_addr: self.hardware_address(),
                    source_protocol_addr: self.ip.address(),
                    target_hardware_addr: source_hardware_addr,
                    target_protocol_addr: source_protocol_addr,
                };

                Self::send_to(
                    &mut self.inner,
                    source_hardware_addr,
                    response.buffer_len(),
                    |buf| response.emit(&mut ArpPacket::new_unchecked(buf)),
                    EthernetProtocol::Arp,
                );
            }

            if self
                .pending_packets
                .peek()
                .is_ok_and(|it| it.0 == &IpAddress::Ipv4(source_protocol_addr))
            {
                self.flush_pending_for(
                    IpAddress::Ipv4(source_protocol_addr),
                    now,
                    EthernetProtocol::Ipv4,
                );
            }
        }
    }
}

impl Device for EthernetDevice {
    fn name(&self) -> &str {
        &self.name
    }

    fn recv(&mut self, buffer: &mut PacketBuffer<()>, timestamp: Instant) -> bool {
        loop {
            let rx_buf = match self.inner.receive() {
                Ok(buf) => buf,
                Err(err) => {
                    if !matches!(err, DevError::Again) {
                        warn!("receive failed: {:?}", err);
                    }
                    return false;
                }
            };
            trace!(
                "RECV {} bytes: {:02X?}",
                rx_buf.packet_len(),
                rx_buf.packet()
            );

            let result = self.handle_frame(rx_buf.packet(), buffer, timestamp);
            self.inner.recycle_rx_buffer(rx_buf).unwrap();
            if result {
                return true;
            }
        }
    }

    fn send(&mut self, next_hop: IpAddress, packet: &[u8], timestamp: Instant) -> bool {
        if let IpAddress::Ipv6(next_hop6) = next_hop {
            if next_hop6.is_multicast() {
                let multicast_mac = Self::ipv6_multicast_to_ethernet(next_hop6);
                Self::send_to(
                    &mut self.inner,
                    multicast_mac,
                    packet.len(),
                    |buf| buf.copy_from_slice(packet),
                    EthernetProtocol::Ipv6,
                );
                return false;
            }

            let need_request = match self.neighbors.get(&next_hop) {
                Some(Some(neighbor)) => {
                    if neighbor.expires_at > timestamp {
                        Self::send_to(
                            &mut self.inner,
                            neighbor.hardware_address,
                            packet.len(),
                            |buf| buf.copy_from_slice(packet),
                            EthernetProtocol::Ipv6,
                        );
                        return false;
                    } else {
                        true
                    }
                }
                Some(None) => false,
                None => true,
            };

            if need_request {
                self.request_ndp(next_hop6);
            }

            if self.pending_packets.is_full() {
                warn!("Pending packets buffer is full, dropping packet");
                return false;
            }
            let Ok(dst_buffer) = self.pending_packets.enqueue(packet.len(), next_hop) else {
                warn!("Failed to enqueue packet in pending packets buffer");
                return false;
            };
            dst_buffer.copy_from_slice(packet);
            return false;
        }

        if next_hop.is_broadcast() || self.ip.broadcast().map(IpAddress::Ipv4) == Some(next_hop) {
            Self::send_to(
                &mut self.inner,
                EthernetAddress::BROADCAST,
                packet.len(),
                |buf| buf.copy_from_slice(packet),
                EthernetProtocol::Ipv4,
            );
            return false;
        }

        let need_request = match self.neighbors.get(&next_hop) {
            Some(Some(neighbor)) => {
                if neighbor.expires_at > timestamp {
                    Self::send_to(
                        &mut self.inner,
                        neighbor.hardware_address,
                        packet.len(),
                        |buf| buf.copy_from_slice(packet),
                        EthernetProtocol::Ipv4,
                    );
                    return false;
                } else {
                    true
                }
            }
            // Request already sent
            Some(None) => false,
            None => true,
        };
        // Only send ARP request if we haven't already requested it
        if need_request {
            self.request_arp(next_hop);
        }
        if self.pending_packets.is_full() {
            warn!("Pending packets buffer is full, dropping packet");
            return false;
        }
        let Ok(dst_buffer) = self.pending_packets.enqueue(packet.len(), next_hop) else {
            warn!("Failed to enqueue packet in pending packets buffer");
            return false;
        };
        dst_buffer.copy_from_slice(packet);
        false
    }

    fn register_waker(&self, waker: &Waker) {
        if let Some(irq) = self.inner.irq_num() {
            register_irq_waker(irq, waker);
        }
    }
}
