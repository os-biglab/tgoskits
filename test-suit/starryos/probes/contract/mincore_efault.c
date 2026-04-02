#include <errno.h>
#include <stdio.h>
#include <sys/mman.h>
int main(void)
{
	void *p = mmap(NULL, 4096, PROT_READ, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
	errno = 0;
	int r = mincore(p, 4096, (unsigned char *)(void *)1);
	int e = errno;
	dprintf(1, "CASE mincore.efault ret=%d errno=%d note=handwritten\n", r, e);
	munmap(p, 4096);
	return 0;
}
