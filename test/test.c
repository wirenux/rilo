/* block
   comment */
// line comment
#include <stdio.h>
#define MAX 10

struct P { int x; };

int main(void) {
    int n = 42, h = 0xFF, v1 = 3;
    float f = 3.14f;
    char c = '\n';
    char *s = "hi \"you\"\t42 // not a comment";

    if (n > 0) return 0;
    for (int i = 0; i < MAX; i++) continue;
    printf("%d\n", n);
    return 1;
}
