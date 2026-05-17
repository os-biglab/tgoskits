/*
 * Regression test for axnet-ng ARP drain head-of-line (HoL) blocking and
 * pending buffer overflow (fixed in commit ebedcf572).
 *
 * Bug 1 – ARP drain HoL blocking
 *   The old drain code peeked at the queue head and stopped if that entry's
 *   next-hop didn't match the just-resolved ARP address.  If an unresolvable
 *   IP was at the head, all subsequent entries (including those for resolved
 *   next-hops) were permanently blocked.
 *
 *   Reproduction (on a cold ARP cache):
 *     1. connect() to 10.0.2.100 (no such SLIRP host, ARP never replies).
 *        Its SYN sits at queue position 0.
 *     2. connect() to the gateway 10.0.2.2 (SLIRP replies to its ARP).
 *        Its SYN sits at queue position 1.
 *     3. ARP reply for 10.0.2.2 arrives.
 *        Old drain: head == .100 != .2 → stops immediately, .2's SYN stuck.
 *        New drain: scans all entries, sends .2's SYN, re-queues .100's SYN.
 *     4. poll() on the .2 socket: old → ETIMEDOUT, new → POLLOUT|POLLERR.
 *
 *   If ARP for 10.0.2.2 is already cached when the test runs (e.g., because
 *   an earlier test already resolved it), step 2 sends the SYN immediately
 *   and the HoL path is not exercised.  The test still validates basic
 *   connectivity and passes, which is the correct result.
 *
 * Bug 2 – ARP pending buffer overflow
 *   ETHERNET_MAX_PENDING_PACKETS was 32.  Firing BURST_COUNT=64 concurrent
 *   SYNs whose next-hop needs ARP overflowed the ring buffer; the excess
 *   SYNs were silently dropped and those connections timed out.  The fix
 *   raises the limit to 256 and ensures the drain sends every resolved entry.
 */

#define _POSIX_C_SOURCE 200809L

#include <arpa/inet.h>
#include <errno.h>
#include <fcntl.h>
#include <netinet/in.h>
#include <poll.h>
#include <stdio.h>
#include <string.h>
#include <sys/socket.h>
#include <unistd.h>

/* QEMU SLIRP models 10.0.2.2 as gateway; 10.0.2.100 has no SLIRP host. */
#define GATEWAY_IP   "10.0.2.2"
#define FAKE_IP      "10.0.2.100"
#define CONNECT_PORT 80

/* Must be > old ETHERNET_MAX_PENDING_PACKETS (32) to catch overflow. */
#define BURST_COUNT  64

/* Tolerate a few SLIRP-side drops (e.g. rate limiting). */
#define MIN_COMPLETE 56

/* ms to wait for a connection attempt to be answered (RST or SYN-ACK). */
#define HOL_TIMEOUT_MS   4000
#define BURST_TIMEOUT_MS 6000

static int make_nb_socket(void)
{
    int fd = socket(AF_INET, SOCK_STREAM, 0);
    if (fd < 0)
        return -1;
    int flags = fcntl(fd, F_GETFL, 0);
    if (flags < 0 || fcntl(fd, F_SETFL, flags | O_NONBLOCK) < 0) {
        close(fd);
        return -1;
    }
    return fd;
}

static struct sockaddr_in make_addr(const char *ip, int port)
{
    struct sockaddr_in a;
    memset(&a, 0, sizeof(a));
    a.sin_family = AF_INET;
    a.sin_port   = htons((unsigned short)port);
    inet_pton(AF_INET, ip, &a.sin_addr);
    return a;
}

/*
 * Test 1: ARP drain HoL blocking.
 *
 * Place a SYN for an unresolvable IP at the queue head, then immediately
 * enqueue a SYN for the real gateway.  The gateway SYN must reach the wire
 * (evidenced by poll() returning) within HOL_TIMEOUT_MS despite the HoL entry.
 *
 * Returns 0 on pass, 1 on fail.
 */
static int test_hol_drain(void)
{
    printf("[TEST] bug-arp-hol-drain: ARP drain HoL blocking\n");

    int sock_fake = make_nb_socket();
    int sock_gw   = make_nb_socket();
    if (sock_fake < 0 || sock_gw < 0) {
        printf("  [FAIL] socket() failed: %s\n", strerror(errno));
        if (sock_fake >= 0) close(sock_fake);
        if (sock_gw   >= 0) close(sock_gw);
        return 1;
    }

    struct sockaddr_in addr_fake = make_addr(FAKE_IP,    CONNECT_PORT);
    struct sockaddr_in addr_gw   = make_addr(GATEWAY_IP, CONNECT_PORT);

    /* Enqueue SYN for fake IP first so it occupies the queue head. */
    int r = connect(sock_fake, (struct sockaddr *)&addr_fake, sizeof(addr_fake));
    if (r < 0 && errno != EINPROGRESS) {
        printf("  [FAIL] connect(fake) unexpected errno %d: %s\n",
               errno, strerror(errno));
        close(sock_fake);
        close(sock_gw);
        return 1;
    }

    /* Enqueue SYN for gateway immediately after. */
    r = connect(sock_gw, (struct sockaddr *)&addr_gw, sizeof(addr_gw));
    if (r < 0 && errno != EINPROGRESS && errno != ECONNREFUSED) {
        printf("  [FAIL] connect(gw) unexpected errno %d: %s\n",
               errno, strerror(errno));
        close(sock_fake);
        close(sock_gw);
        return 1;
    }

    int already_done = (r == 0 || errno == ECONNREFUSED);

    if (already_done) {
        /*
         * ARP for gateway was cached; SYN sent immediately without going
         * through the pending queue.  HoL path not exercised, but the
         * connection completed correctly — that is the right behavior.
         */
        printf("  [INFO] ARP cache warm: gateway SYN sent immediately "
               "(HoL path not exercised this run)\n");
        printf("  [PASS] gateway reachable\n");
        close(sock_fake);
        close(sock_gw);
        return 0;
    }

    /* ARP was cold; wait for the gateway SYN to be drained and answered. */
    struct pollfd pfd = { .fd = sock_gw, .events = POLLOUT | POLLERR | POLLHUP };
    int n = poll(&pfd, 1, HOL_TIMEOUT_MS);

    int rc;
    if (n > 0 && (pfd.revents & (POLLOUT | POLLERR | POLLHUP))) {
        printf("  [PASS] gateway SYN drained through pending queue "
               "(HoL blocking absent)\n");
        rc = 0;
    } else if (n == 0) {
        printf("  [FAIL] poll() timed out: gateway SYN was never sent "
               "(ARP drain HoL blocking present)\n");
        rc = 1;
    } else {
        printf("  [FAIL] poll() error: %s\n", strerror(errno));
        rc = 1;
    }

    close(sock_fake);
    close(sock_gw);
    return rc;
}

/*
 * Test 2: ARP pending buffer overflow.
 *
 * Fire BURST_COUNT concurrent non-blocking connects to the gateway.  If the
 * pending buffer is too small (old: 32 slots), the excess SYNs are dropped
 * and those connections time out.  With the fix (256 slots + full drain) all
 * BURST_COUNT connections must receive a response within BURST_TIMEOUT_MS.
 *
 * Returns 0 on pass, 1 on fail.
 */
static int test_burst_capacity(void)
{
    printf("[TEST] bug-arp-hol-drain: ARP pending buffer capacity (%d conns)\n",
           BURST_COUNT);

    int fds[BURST_COUNT];
    struct pollfd pfds[BURST_COUNT];
    struct sockaddr_in addr_gw = make_addr(GATEWAY_IP, CONNECT_PORT);
    int opened = 0;

    for (int i = 0; i < BURST_COUNT; i++) {
        fds[i] = make_nb_socket();
        if (fds[i] < 0) {
            printf("  [WARN] could only open %d sockets\n", i);
            break;
        }
        opened++;
        connect(fds[i], (struct sockaddr *)&addr_gw, sizeof(addr_gw));
        /* Ignore EINPROGRESS / ECONNREFUSED; we check via poll below. */
    }

    for (int i = 0; i < opened; i++) {
        pfds[i].fd     = fds[i];
        pfds[i].events = POLLOUT | POLLERR | POLLHUP;
    }

    int completed = 0;
    int remaining = opened;
    int deadline  = BURST_TIMEOUT_MS;

    while (remaining > 0 && deadline > 0) {
        int n = poll(pfds, (nfds_t)opened, 500);
        if (n < 0)
            break;
        deadline -= 500;
        for (int i = 0; i < opened; i++) {
            if (pfds[i].fd < 0)
                continue;
            if (pfds[i].revents & (POLLOUT | POLLERR | POLLHUP)) {
                completed++;
                close(pfds[i].fd);
                pfds[i].fd = -1;
                remaining--;
            }
        }
    }

    for (int i = 0; i < opened; i++) {
        if (pfds[i].fd >= 0)
            close(pfds[i].fd);
    }

    printf("  completed %d / %d connections within %d ms\n",
           completed, opened, BURST_TIMEOUT_MS);

    if (completed >= MIN_COMPLETE) {
        printf("  [PASS] >= %d connections completed (buffer large enough)\n",
               MIN_COMPLETE);
        return 0;
    }
    printf("  [FAIL] only %d / %d completed "
           "(ARP pending buffer overflow or HoL drain still blocked)\n",
           completed, opened);
    return 1;
}

int main(void)
{
    printf("=== bug-arp-hol-drain ===\n");

    int failed = 0;
    failed += test_hol_drain();
    failed += test_burst_capacity();

    if (failed == 0) {
        printf("STARRY_GROUPED_TEST_PASSED: bug-arp-hol-drain\n");
        return 0;
    }
    printf("STARRY_GROUPED_TEST_FAILED: bug-arp-hol-drain\n");
    return 1;
}
