// #include <stdio.h>
#define macro macro + 1
#define func(x) x+1

#if defined(func)
is this expanded
#endif

int main() {
  macro + 123;
  // func(2);
}
