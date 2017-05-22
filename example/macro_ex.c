/***** WARNING *****
 * this code can't be compiled.
 * because in line.20 'macro' will be replaced with 'macro + 1'.
 * there is not such declared variable.
 */

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
