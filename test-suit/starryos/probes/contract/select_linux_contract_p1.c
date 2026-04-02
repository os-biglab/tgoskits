#include <errno.h>
#include <stdio.h>
#include <sys/select.h>
int main(void)
{
	errno = 0;
	int r = select(-1, NULL, NULL, NULL, NULL);
	int e = errno;
	dprintf(1, "CASE select.einval ret=%d errno=%d note=handwritten\n", r, e);
	return 0;
}
