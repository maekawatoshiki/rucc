// #include <stdio.h>
#define macro macro + 1
#define func(x) x+1

#if defined(func)
void this_func_will_be_expanded() {
}
#endif

#ifndef macro
void not_expanded() {
}
#endif

int main() {
  macro + 123;
  func(2);
}
