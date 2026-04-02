#include <errno.h>
#include <stdio.h>
#include <sys/mman.h>
#include <sys/syscall.h>
#include <unistd.h>
#ifndef SYS_mlock2
#define SYS_mlock2 284
#endif
int main(void)
{
	void *p = mmap(NULL, 4096, PROT_READ, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
	errno = 0;
	long r = syscall(SYS_mlock2, p, 4096UL, (unsigned int)-1);
	int e = errno;
	dprintf(1, "CASE mlock2.einval ret=%ld errno=%d note=handwritten\n", r, e);
	munmap(p, 4096);
	return 0;
}
