#include <stdio.h>

int main() {
  int q[] = {123, 456, 789};
  int *has_ptr_to_local_var[] = {q};
  char s[][8] = {"hello", "rucc", "world"};
  int a[2];
  a[0] = 12;
  a[1] = 23;
  a[0] = a[0] + a[1];
  printf("%d %d\n", a[0], 0[a]);
  for(int i = 0; i < 3; i++) 
    puts(s[i]);
}
