/*
 * net-irq: verify that a blocking recv() on a UDP socket is correctly
 * woken up by the virtio-net device interrupt after a packet arrives.
 *
 * Test flow:
 *   1. Create a UDP socket and connect() it to QEMU SLIRP's DNS (10.0.2.3:53).
 *      connect() on a UDP socket does not send any packets; it merely records
 *      the remote address so that subsequent send()/recv() know the peer.
 *      It also auto-binds an ephemeral local port.
 *   2. send() a DNS query.  send() returns immediately; QEMU has NOT yet
 *      placed the DNS reply in the virtio-net RX ring.
 *   3. Call blocking recv() with no timeout.
 *      Because the reply is not ready, poll_io parks the task (Poll::Pending)
 *      and registers an IRQ waker for the virtio-net device interrupt vector.
 *   4. QEMU processes the DNS query and delivers the reply into the
 *      virtio-net RX ring, then fires a PCI interrupt.
 *
 * PASS: the PCI interrupt vector matches the registered waker → task wakes →
 *       recv() returns data → prints "TEST PASSED".
 *
 * FAIL (timeout): the PCI interrupt vector does NOT match the registered
 *       waker → task stays parked → test runner timeout fires → FAIL.
 */

#include <arpa/inet.h>
#include <netinet/in.h>
#include <stdio.h>
#include <string.h>
#include <sys/socket.h>
#include <unistd.h>

/* Minimal DNS query: A-record lookup for "localhost"
 * QEMU SLIRP's built-in resolver handles this locally — no internet needed.
 * Wire format: 12-byte header + QNAME labels + QTYPE + QCLASS
 */
static const unsigned char DNS_QUERY[] = {
    0xAB, 0x01,              /* Transaction ID */
    0x01, 0x00,              /* Flags: standard query, RD=1 */
    0x00, 0x01,              /* QDCOUNT = 1 */
    0x00, 0x00,              /* ANCOUNT = 0 */
    0x00, 0x00,              /* NSCOUNT = 0 */
    0x00, 0x00,              /* ARCOUNT = 0 */
    0x09, 'l','o','c','a','l','h','o','s','t', 0x00, /* QNAME */
    0x00, 0x01,              /* QTYPE  = A */
    0x00, 0x01,              /* QCLASS = IN */
};

int main(void) {
    int fd = socket(AF_INET, SOCK_DGRAM, 0);
    if (fd < 0) {
        perror("socket");
        return 1;
    }

    struct sockaddr_in dns = {0};
    dns.sin_family      = AF_INET;
    dns.sin_port        = htons(53);
    dns.sin_addr.s_addr = inet_addr("10.0.2.3"); /* QEMU SLIRP DNS */

    /* connect() records the remote peer and auto-binds a local port.
     * This is required so that the subsequent plain recv() (which has no
     * address argument) knows which peer to expect data from. */
    if (connect(fd, (struct sockaddr *)&dns, sizeof(dns)) < 0) {
        perror("connect");
        close(fd);
        return 1;
    }

    /* send() the DNS query.  Returns immediately; QEMU hasn't responded yet. */
    if (send(fd, DNS_QUERY, sizeof(DNS_QUERY), 0) < 0) {
        perror("send");
        close(fd);
        return 1;
    }
    printf("DNS query sent. Calling blocking recv() (no timeout)...\n");

    /* Blocking recv() — no SO_RCVTIMEO, no MSG_DONTWAIT.
     * If the virtio-net IRQ waker fires at the wrong APIC vector the task
     * stays parked and the test runner's global timeout catches the hang. */
    char buf[512];
    ssize_t n = recv(fd, buf, sizeof(buf), 0);
    close(fd);

    if (n > 0) {
        printf("Received %zd-byte DNS reply.\n", n);
        printf("TEST PASSED\n");
        return 0;
    }

    perror("recv");
    printf("TEST FAILED\n");
    return 1;
}
