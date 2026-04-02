#include <errno.h>
#include <stdio.h>
#include <sys/syscall.h>
#include <unistd.h>
int main(void)
{
	void *b = (void *)syscall(SYS_brk, (void *)0);
	errno = 0;
	void *b2 = (void *)syscall(SYS_brk, b);
	int e = errno;
	int r = (b2 == b) ? 0 : -1;
	dprintf(1, "CASE brk_increment.smoke ret=%d errno=%d note=handwritten\n", r, e);
	return 0;
}
