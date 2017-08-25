#include <stdio.h>

int main() {
  for(int i = 0; i < 3; i++) {
    switch(i) {
      case 0:
        printf("i is 0\n");
        break;
      case 1:
      case 2:
        puts("i is 1 or 2");
      default:
        puts("this is default block");
    }
  }
  puts("finish");
  return 0;
}
