#include <stdio.h>

int main() {
  for(int i = 0; i < 10; i++) {
    if(i == 8) break;
    if(i & 1) continue;
    printf("%d ", i);
  }
  puts("");
  for(int i = 0; i < 10; i++) {
    for(int k = 0; k < 10; k++) {
      printf("%d\n", k);
      if(k == 2) break;
    }
    printf("%d\n", i);
    if(i == 3) break;
  }
}
