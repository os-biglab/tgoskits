#include <errno.h>
#include <stdio.h>
#include <sys/syscall.h>
#include <unistd.h>
int main(void)
{
	errno = 0;
	long r = syscall(SYS_pselect6, -1, NULL, NULL, NULL, NULL, NULL);
	int e = errno;
	dprintf(1, "CASE pselect6.einval ret=%ld errno=%d note=handwritten\n", r, e);
	return 0;
}
