#include <stdio.h>

int main() {
  for(int i = 0; i < 10; i++) {
    if(i == 8) break;
    if(i & 1) continue;
    printf("%d ", i);
  }
  puts("");
}
