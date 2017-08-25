#include <stdio.h>
#define SHOW_INT(x) do { printf("%s = %d\n", #x, x); } while(0);
#define SHOW_STR(x) do { printf("%s = %s\n", #x, x); } while(0);

struct user_t {
  char *name;
  size_t age;
};

struct A {
  struct user_t u;
  int i, k;
} a = {{"name", 12}, 12, 23};

int main() {
  SHOW_INT(a.i);
  SHOW_INT(a.k);
  SHOW_INT(a.u.age);
  SHOW_STR(a.u.name);
  return 0;
}
