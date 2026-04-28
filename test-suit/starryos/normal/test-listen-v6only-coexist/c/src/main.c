#define _GNU_SOURCE
#include "test_framework.h"

#include <arpa/inet.h>
#include <netinet/in.h>
#include <sys/socket.h>
#include <sys/wait.h>
#include <unistd.h>

#define BASE_PORT 28110
#define MSG_V4 "v4"
#define MSG_V6 "v6"

static void wait_child(pid_t pid, const char *label)
{
    int status = 0;
    pid_t r;
    do {
        r = waitpid(pid, &status, 0);
    } while (r == -1 && errno == EINTR);
    CHECK(WIFEXITED(status) && WEXITSTATUS(status) == 0, label);
}

static int make_v6_listener(int port, int v6only)
{
    int fd = socket(AF_INET6, SOCK_STREAM, 0);
    CHECK(fd >= 0, "socket(AF_INET6, SOCK_STREAM)");
    if (fd < 0) {
        return -1;
    }

    int one = 1;
    setsockopt(fd, SOL_SOCKET, SO_REUSEADDR, &one, sizeof(one));
    CHECK_RET(setsockopt(fd, IPPROTO_IPV6, IPV6_V6ONLY, &v6only, sizeof(v6only)), 0,
              "setsockopt(IPV6_V6ONLY)");

    struct sockaddr_in6 addr = {0};
    addr.sin6_family = AF_INET6;
    addr.sin6_port = htons(port);
    addr.sin6_addr = in6addr_any;

    CHECK_RET(bind(fd, (struct sockaddr *)&addr, sizeof(addr)), 0,
              "bind(AF_INET6, ::, port)");
    CHECK_RET(listen(fd, 8), 0, "listen(AF_INET6)");
    return fd;
}

static int make_v6_listener_at(int port, int v6only, const struct in6_addr *addr)
{
    int fd = socket(AF_INET6, SOCK_STREAM, 0);
    CHECK(fd >= 0, "socket(AF_INET6, SOCK_STREAM)");
    if (fd < 0) {
        return -1;
    }

    int one = 1;
    setsockopt(fd, SOL_SOCKET, SO_REUSEADDR, &one, sizeof(one));
    CHECK_RET(setsockopt(fd, IPPROTO_IPV6, IPV6_V6ONLY, &v6only, sizeof(v6only)), 0,
              "setsockopt(IPV6_V6ONLY)");

    struct sockaddr_in6 addr6 = {0};
    addr6.sin6_family = AF_INET6;
    addr6.sin6_port = htons(port);
    addr6.sin6_addr = *addr;

    CHECK_RET(bind(fd, (struct sockaddr *)&addr6, sizeof(addr6)), 0,
              "bind(AF_INET6, addr, port)");
    CHECK_RET(listen(fd, 8), 0, "listen(AF_INET6)");
    return fd;
}

static int make_v4_listener(int port)
{
    int fd = socket(AF_INET, SOCK_STREAM, 0);
    CHECK(fd >= 0, "socket(AF_INET, SOCK_STREAM)");
    if (fd < 0) {
        return -1;
    }

    int one = 1;
    setsockopt(fd, SOL_SOCKET, SO_REUSEADDR, &one, sizeof(one));

    struct sockaddr_in addr = {0};
    addr.sin_family = AF_INET;
    addr.sin_port = htons(port);
    addr.sin_addr.s_addr = htonl(INADDR_ANY);

    CHECK_RET(bind(fd, (struct sockaddr *)&addr, sizeof(addr)), 0,
              "bind(AF_INET, 0.0.0.0, same port)");
    CHECK_RET(listen(fd, 8), 0, "listen(AF_INET)");
    return fd;
}

static int make_v4_listener_at(int port, uint32_t addr)
{
    int fd = socket(AF_INET, SOCK_STREAM, 0);
    CHECK(fd >= 0, "socket(AF_INET, SOCK_STREAM)");
    if (fd < 0) {
        return -1;
    }

    int one = 1;
    setsockopt(fd, SOL_SOCKET, SO_REUSEADDR, &one, sizeof(one));

    struct sockaddr_in addr4 = {0};
    addr4.sin_family = AF_INET;
    addr4.sin_port = htons(port);
    addr4.sin_addr.s_addr = addr;

    CHECK_RET(bind(fd, (struct sockaddr *)&addr4, sizeof(addr4)), 0,
              "bind(AF_INET, addr, same port)");
    CHECK_RET(listen(fd, 8), 0, "listen(AF_INET)");
    return fd;
}

static void test_v6only_and_v4_can_coexist(void)
{
    int v6_fd = make_v6_listener(BASE_PORT, 1);
    if (v6_fd < 0) {
        __fail++;
        return;
    }

    int v4_fd = make_v4_listener(BASE_PORT);
    if (v4_fd < 0) {
        close(v6_fd);
        __fail++;
        return;
    }

    pid_t v4_client = fork();
    if (v4_client == 0) {
        int c = socket(AF_INET, SOCK_STREAM, 0);
        if (c < 0) {
            _exit(2);
        }
        struct sockaddr_in dst = {0};
        dst.sin_family = AF_INET;
        dst.sin_port = htons(BASE_PORT);
        dst.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
        if (connect(c, (struct sockaddr *)&dst, sizeof(dst)) != 0) {
            close(c);
            _exit(3);
        }
        if (send(c, MSG_V4, sizeof(MSG_V4), 0) != (ssize_t)sizeof(MSG_V4)) {
            close(c);
            _exit(4);
        }
        close(c);
        _exit(0);
    }

    struct sockaddr_in peer4 = {0};
    socklen_t p4len = sizeof(peer4);
    int v4_conn = accept(v4_fd, (struct sockaddr *)&peer4, &p4len);
    CHECK(v4_conn >= 0, "AF_INET listener accepts IPv4 connection");
    if (v4_conn >= 0) {
        char buf[16] = {0};
        ssize_t n = recv(v4_conn, buf, sizeof(buf), 0);
        CHECK(n == (ssize_t)sizeof(MSG_V4), "AF_INET listener receives payload");
        close(v4_conn);
    }

    wait_child(v4_client, "IPv4 client finished successfully");

    pid_t v6_client = fork();
    if (v6_client == 0) {
        int c = socket(AF_INET6, SOCK_STREAM, 0);
        if (c < 0) {
            _exit(5);
        }
        struct sockaddr_in6 dst = {0};
        dst.sin6_family = AF_INET6;
        dst.sin6_port = htons(BASE_PORT);
        dst.sin6_addr = in6addr_loopback;
        if (connect(c, (struct sockaddr *)&dst, sizeof(dst)) != 0) {
            close(c);
            _exit(6);
        }
        if (send(c, MSG_V6, sizeof(MSG_V6), 0) != (ssize_t)sizeof(MSG_V6)) {
            close(c);
            _exit(7);
        }
        close(c);
        _exit(0);
    }

    struct sockaddr_in6 peer6 = {0};
    socklen_t p6len = sizeof(peer6);
    int v6_conn = accept(v6_fd, (struct sockaddr *)&peer6, &p6len);
    CHECK(v6_conn >= 0, "AF_INET6(V6ONLY) listener accepts IPv6 connection");
    if (v6_conn >= 0) {
        char buf[16] = {0};
        ssize_t n = recv(v6_conn, buf, sizeof(buf), 0);
        CHECK(n == (ssize_t)sizeof(MSG_V6), "AF_INET6(V6ONLY) listener receives payload");
        close(v6_conn);
    }

    wait_child(v6_client, "IPv6 client finished successfully");

    close(v4_fd);
    close(v6_fd);
}

static void test_v6_loopback_and_v4_loopback_can_coexist(void)
{
    int v6_fd = make_v6_listener_at(BASE_PORT + 1, 0, &in6addr_loopback);
    if (v6_fd < 0) {
        __fail++;
        return;
    }

    int v4_fd = make_v4_listener_at(BASE_PORT + 1, htonl(INADDR_LOOPBACK));
    if (v4_fd < 0) {
        close(v6_fd);
        __fail++;
        return;
    }

    pid_t v4_client = fork();
    if (v4_client == 0) {
        int c = socket(AF_INET, SOCK_STREAM, 0);
        if (c < 0) {
            _exit(2);
        }
        struct sockaddr_in dst = {0};
        dst.sin_family = AF_INET;
        dst.sin_port = htons(BASE_PORT + 1);
        dst.sin_addr.s_addr = htonl(INADDR_LOOPBACK);
        if (connect(c, (struct sockaddr *)&dst, sizeof(dst)) != 0) {
            close(c);
            _exit(3);
        }
        if (send(c, MSG_V4, sizeof(MSG_V4), 0) != (ssize_t)sizeof(MSG_V4)) {
            close(c);
            _exit(4);
        }
        close(c);
        _exit(0);
    }

    struct sockaddr_in peer4 = {0};
    socklen_t p4len = sizeof(peer4);
    int v4_conn = accept(v4_fd, (struct sockaddr *)&peer4, &p4len);
    CHECK(v4_conn >= 0, "AF_INET listener accepts 127.0.0.1 connection");
    if (v4_conn >= 0) {
        char buf[16] = {0};
        ssize_t n = recv(v4_conn, buf, sizeof(buf), 0);
        CHECK(n == (ssize_t)sizeof(MSG_V4), "AF_INET loopback listener receives payload");
        close(v4_conn);
    }

    wait_child(v4_client, "IPv4 loopback client finished successfully");

    pid_t v6_client = fork();
    if (v6_client == 0) {
        int c = socket(AF_INET6, SOCK_STREAM, 0);
        if (c < 0) {
            _exit(5);
        }
        struct sockaddr_in6 dst = {0};
        dst.sin6_family = AF_INET6;
        dst.sin6_port = htons(BASE_PORT + 1);
        dst.sin6_addr = in6addr_loopback;
        if (connect(c, (struct sockaddr *)&dst, sizeof(dst)) != 0) {
            close(c);
            _exit(6);
        }
        if (send(c, MSG_V6, sizeof(MSG_V6), 0) != (ssize_t)sizeof(MSG_V6)) {
            close(c);
            _exit(7);
        }
        close(c);
        _exit(0);
    }

    struct sockaddr_in6 peer6 = {0};
    socklen_t p6len = sizeof(peer6);
    int v6_conn = accept(v6_fd, (struct sockaddr *)&peer6, &p6len);
    CHECK(v6_conn >= 0, "AF_INET6(::1) listener accepts IPv6 connection");
    if (v6_conn >= 0) {
        char buf[16] = {0};
        ssize_t n = recv(v6_conn, buf, sizeof(buf), 0);
        CHECK(n == (ssize_t)sizeof(MSG_V6), "AF_INET6(::1) listener receives payload");
        close(v6_conn);
    }

    wait_child(v6_client, "IPv6 loopback client finished successfully");

    close(v4_fd);
    close(v6_fd);
}

int main(void)
{
    TEST_START("listen table coexistence: AF_INET6(V6ONLY) + AF_INET same port");

    test_v6only_and_v4_can_coexist();
    test_v6_loopback_and_v4_loopback_can_coexist();

    TEST_DONE();
}
