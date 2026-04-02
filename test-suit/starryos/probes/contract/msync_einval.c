#include <errno.h>
#include <stdio.h>
#include <sys/mman.h>
int main(void)
{
	errno = 0;
	int r = msync((void *)1, 0, MS_SYNC);
	int e = errno;
	dprintf(1, "CASE msync.einval ret=%d errno=%d note=handwritten\n", r, e);
	return 0;
}
