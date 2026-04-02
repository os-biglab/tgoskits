/* Hand-written contract probe: ioctl(2) on invalid fd -> EBADF. */
#include <errno.h>
#include <stdio.h>
#include <sys/ioctl.h>

int main(void)
{
	errno = 0;
	int r = ioctl(-1, FIONREAD, 0);
	int e = errno;
	dprintf(1, "CASE ioctl.badfd ret=%d errno=%d note=handwritten\n", r, e);
	return 0;
}
