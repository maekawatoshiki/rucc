#include <stdio.h>

int main() {
  // TODO: this is for coverage. many other operators will be checked in the future.
  printf("%d\n", 1 + 2);
  printf("%d\n", 2 - 1);
  printf("%d\n", 2 * 3);
  printf("%d\n", 4 / 2);
  printf("%d\n", 4 % 3);
  printf("%d\n", 1 & 2);
  printf("%d\n", 1 | 2);
  printf("%d\n", 1 ^ 2);
  printf("%d\n", 1 && 0);
  printf("%d\n", 1 || 0);
  printf("%d\n", 5 == 5);
  printf("%d\n", 1 != 1);
  printf("%d\n", 3 < 4);
  printf("%d\n", 3 > 4);
  printf("%d\n", 3 <= 4);
  printf("%d\n", 3 >= 4);
  printf("%d\n", 1 << 4);
  printf("%d\n", 16 >> 4);
  printf("%d\n", !0);
  printf("%d\n", ~123);
  printf("%d\n", +12);
  printf("%d\n", -34);
  int i = 0, *a = &i;
  printf("%d\n", ++i);
  printf("%d\n", --i);
  printf("%d\n", i++);
  printf("%d\n", i--);
  printf("%d\n", *a);
  i += 10;
  i -= 10;
  i *= 10;
  i /= 10;
  i %= 10;
  i <<= 10;
  i >>= 10;
  i &= 10;
  i |= 10;
  return 0;
}
