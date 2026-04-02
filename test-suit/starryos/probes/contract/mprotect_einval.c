#include <errno.h>
#include <stdio.h>
#include <sys/mman.h>
int main(void)
{
	void *p = mmap(NULL, 4096, PROT_READ, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
	errno = 0;
	int r = mprotect(p, 4096, PROT_READ | PROT_GROWSUP);
	int e = errno;
	dprintf(1, "CASE mprotect.einval ret=%d errno=%d note=handwritten\n", r, e);
	munmap(p, 4096);
	return 0;
}
