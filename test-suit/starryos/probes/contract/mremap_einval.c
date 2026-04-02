#include <errno.h>
#include <stdio.h>
#include <sys/mman.h>
#include <sys/syscall.h>
#include <unistd.h>
#ifndef SYS_mremap
#define SYS_mremap 216
#endif
#ifndef MREMAP_MAYMOVE
#define MREMAP_MAYMOVE 1
#endif
#ifndef MREMAP_FIXED
#define MREMAP_FIXED 2
#endif
int main(void)
{
	void *p = mmap(NULL, 4096, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
	errno = 0;
	long r = (long)syscall(SYS_mremap, p, 4096UL, 4096UL, MREMAP_FIXED | MREMAP_MAYMOVE, (void *)1);
	int e = errno;
	dprintf(1, "CASE mremap.einval ret=%ld errno=%d note=handwritten\n", r, e);
	munmap(p, 4096);
	return 0;
}
