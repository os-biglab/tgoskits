#include <errno.h>
#include <stdio.h>
#include <sys/syscall.h>
#include <unistd.h>
int main(void)
{
	errno = 0;
	long r = syscall(SYS_epoll_create1, -1);
	int e = errno;
	dprintf(1, "CASE epoll_create1.einval ret=%ld errno=%d note=handwritten\n", r, e);
	return 0;
}
