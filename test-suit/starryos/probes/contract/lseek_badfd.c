/* Hand-written contract probe: lseek(2) on invalid fd -> EBADF. */
#include <errno.h>
#include <stdio.h>
#include <unistd.h>

int main(void)
{
	errno = 0;
	off_t r = lseek(-1, 0, SEEK_SET);
	int e = errno;
	dprintf(1, "CASE lseek.badfd ret=%lld errno=%d note=handwritten\n", (long long)r, e);
	return 0;
}
