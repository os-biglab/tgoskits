#include <errno.h>
#include <stdio.h>
#include <sys/mman.h>
int main(void)
{
	errno = 0;
	int r = munmap((void *)1, 0);
	int e = errno;
	dprintf(1, "CASE munmap.einval ret=%d errno=%d note=handwritten\n", r, e);
	return 0;
}
