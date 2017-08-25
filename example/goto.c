#include <stdio.h>

int main() {
  int i = 0; 
loop:
  printf("%d\n", i++);
  if(i == 5) {
    goto break_loop;
  }
  goto loop;
break_loop:
  return 0;
}
