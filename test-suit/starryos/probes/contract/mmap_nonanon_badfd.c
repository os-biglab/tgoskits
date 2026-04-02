#include <errno.h>
#include <stdio.h>
#include <sys/mman.h>
int main(void)
{
	errno = 0;
	void *p = mmap(NULL, 4096, PROT_READ, MAP_PRIVATE, -1, 0);
	long r = (long)(unsigned long)p;
	int e = errno;
	dprintf(1, "CASE mmap.nonanon_badfd ret=%ld errno=%d note=handwritten\n", r, e);
	return 0;
}
