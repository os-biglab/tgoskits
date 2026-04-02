#include <errno.h>
#include <stdio.h>
#include <sys/mman.h>
int main(void)
{
	errno = 0;
	int r = madvise((void *)1, 4096, MADV_NORMAL);
	int e = errno;
	dprintf(1, "CASE madvise.einval ret=%d errno=%d note=handwritten\n", r, e);
	return 0;
}
