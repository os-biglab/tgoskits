use alloc::{boxed::Box, collections::VecDeque, sync::Arc, vec};

use ax_errno::{AxError, AxResult};
use ax_sync::Mutex;
use smoltcp::{
    iface::{SocketHandle, SocketSet},
    socket::tcp::{self, SocketBuffer, State},
    wire::{IpAddress, IpEndpoint, IpListenEndpoint},
};

use crate::{
    SOCKET_SET,
    consts::{LISTEN_QUEUE_SIZE, TCP_RX_BUF_LEN, TCP_TX_BUF_LEN},
};

const PORT_NUM: usize = 65536;

struct ListenTableEntryInner {
    listen_endpoint: IpListenEndpoint,
    syn_queue: VecDeque<SocketHandle>,
    is_ipv6: bool,
    v6only: bool,
}

impl ListenTableEntryInner {
    pub fn new(listen_endpoint: IpListenEndpoint, is_ipv6: bool, v6only: bool) -> Self {
        Self {
            listen_endpoint,
            syn_queue: VecDeque::with_capacity(LISTEN_QUEUE_SIZE),
            is_ipv6,
            v6only,
        }
    }
}

impl Drop for ListenTableEntryInner {
    fn drop(&mut self) {
        for &handle in &self.syn_queue {
            SOCKET_SET.remove(handle);
        }
    }
}

#[derive(Default)]
struct ListenTablePortEntry {
    v4: Option<Box<ListenTableEntryInner>>,
    v6: Option<Box<ListenTableEntryInner>>,
}

pub struct ListenTable {
    tcp: Box<[Arc<Mutex<ListenTablePortEntry>>]>,
}

impl ListenTable {
    pub fn new() -> Self {
        let tcp = unsafe {
            let mut buf = Box::new_uninit_slice(PORT_NUM);
            for i in 0..PORT_NUM {
                buf[i].write(Arc::new(Mutex::new(ListenTablePortEntry::default())));
            }
            buf.assume_init()
        };
        Self { tcp }
    }

    pub fn can_listen(&self, port: u16) -> bool {
        let entry = self.tcp[port as usize].lock();
        entry.v4.is_none() && entry.v6.is_none()
    }

    pub fn listen(
        &self,
        listen_endpoint: IpListenEndpoint,
        is_ipv6: bool,
        v6only: bool,
    ) -> AxResult {
        let port = listen_endpoint.port;
        assert_ne!(port, 0);
        let mut entry = self.tcp[port as usize].lock();

        if is_ipv6 {
            // AF_INET6 dual-stack listener conflicts with both AF_INET and AF_INET6.
            if !v6only {
                if entry.v4.is_some() || entry.v6.is_some() {
                    warn!("socket already listening on port {port}");
                    return Err(AxError::AddrInUse);
                }
            } else if entry.v6.is_some() {
                // AF_INET6 + IPV6_V6ONLY=1 only conflicts with AF_INET6 listener.
                warn!("socket already listening on port {port}");
                return Err(AxError::AddrInUse);
            }

            entry.v6 = Some(Box::new(ListenTableEntryInner::new(
                listen_endpoint,
                true,
                v6only,
            )));
            return Ok(());
        }

        // AF_INET listener conflicts with AF_INET and AF_INET6 dual-stack listener.
        if entry.v4.is_some() || entry.v6.as_ref().is_some_and(|it| !it.v6only) {
            warn!("socket already listening on port {port}");
            return Err(AxError::AddrInUse);
        }

        entry.v4 = Some(Box::new(ListenTableEntryInner::new(
            listen_endpoint,
            false,
            false,
        )));
        Ok(())
    }

    pub fn unlisten(&self, port: u16, is_ipv6: bool) {
        debug!("TCP socket unlisten on {}, ipv6={is_ipv6}", port);
        let mut entry = self.tcp[port as usize].lock();
        if is_ipv6 {
            entry.v6 = None;
        } else {
            entry.v4 = None;
        }
    }

    fn listen_entry(&self, port: u16) -> Arc<Mutex<ListenTablePortEntry>> {
        self.tcp[port as usize].clone()
    }

    pub fn can_accept(&self, port: u16, is_ipv6: bool) -> AxResult<bool> {
        let table = self.listen_entry(port);
        let table = table.lock();
        let entry = if is_ipv6 {
            table.v6.as_ref()
        } else {
            table.v4.as_ref()
        }
        .ok_or_else(|| {
            warn!("accept before listen");
            AxError::InvalidInput
        })?;
        Ok(entry.syn_queue.iter().any(|&handle| is_connected(handle)))
    }

    pub fn accept(&self, port: u16, is_ipv6: bool) -> AxResult<(SocketHandle, bool)> {
        let entry = self.listen_entry(port);
        let mut table = entry.lock();
        let Some(entry) = (if is_ipv6 {
            table.v6.as_mut()
        } else {
            table.v4.as_mut()
        }) else {
            warn!("accept before listen");
            return Err(AxError::InvalidInput);
        };

        let is_ipv6 = entry.is_ipv6;
        let syn_queue: &mut VecDeque<SocketHandle> = &mut entry.syn_queue;
        let idx = syn_queue
            .iter()
            .enumerate()
            .find_map(|(idx, &handle)| is_connected(handle).then_some(idx))
            .ok_or(AxError::WouldBlock)?; // wait for connection
        if idx > 0 {
            warn!(
                "slow SYN queue enumeration: index = {}, len = {}!",
                idx,
                syn_queue.len()
            );
        }
        let handle = syn_queue.swap_remove_front(idx).unwrap();
        // If the connection is reset, return ConnectionReset error
        if is_closed(handle) {
            warn!("accept failed: connection reset");
            Err(AxError::ConnectionReset)
        } else {
            Ok((handle, is_ipv6))
        }
    }

    pub fn incoming_tcp_packet(
        &self,
        src: IpEndpoint,
        dst: IpEndpoint,
        sockets: &mut SocketSet<'_>,
    ) {
        let table = self.listen_entry(dst.port);
        let mut table = table.lock();

        let entry = match dst.addr {
            IpAddress::Ipv4(_) => {
                if table.v4.is_some() {
                    table.v4.as_mut()
                } else {
                    table.v6.as_mut().filter(|it| !it.v6only)
                }
            }
            IpAddress::Ipv6(_) => table.v6.as_mut(),
        };

        if let Some(entry) = entry {
            // IPV6_V6ONLY: reject IPv4 connections on an IPv6-only socket
            if entry.v6only && matches!(dst.addr, IpAddress::Ipv4(_)) {
                return;
            }
            if entry.syn_queue.len() >= LISTEN_QUEUE_SIZE {
                // SYN queue is full, drop the packet
                warn!("SYN queue overflow!");
                return;
            }

            let mut socket = smoltcp::socket::tcp::Socket::new(
                SocketBuffer::new(vec![0; TCP_RX_BUF_LEN]),
                SocketBuffer::new(vec![0; TCP_TX_BUF_LEN]),
            );
            // Stamp the bound endpoint with the concrete destination address from
            // the SYN packet so that getsockname() on accepted sockets returns the
            // correct family/address (e.g. AF_INET6 for ::1 connections).
            socket.set_bound_endpoint(IpListenEndpoint {
                addr: Some(dst.addr),
                port: dst.port,
            });
            if let Err(err) = socket.listen(IpListenEndpoint {
                addr: entry.listen_endpoint.addr,
                port: dst.port,
            }) {
                warn!("Failed to listen on {}: {:?}", entry.listen_endpoint, err);
                return;
            }
            let handle = sockets.add(socket);
            debug!(
                "TCP socket {}: prepare for connection {} -> {}",
                handle, src, entry.listen_endpoint
            );
            entry.syn_queue.push_back(handle);
        }
    }
}

fn is_connected(handle: SocketHandle) -> bool {
    SOCKET_SET.with_socket::<tcp::Socket, _, _>(handle, |socket| {
        !matches!(socket.state(), State::Listen | State::SynReceived)
    })
}

fn is_closed(handle: SocketHandle) -> bool {
    SOCKET_SET
        .with_socket::<tcp::Socket, _, _>(handle, |socket| matches!(socket.state(), State::Closed))
}
