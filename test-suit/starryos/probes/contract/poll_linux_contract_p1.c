#include <errno.h>
#include <poll.h>
#include <stdio.h>
int main(void)
{
	errno = 0;
	int r = poll(NULL, -1, 0);
	int e = errno;
	dprintf(1, "CASE poll.einval ret=%d errno=%d note=handwritten\n", r, e);
	return 0;
}
