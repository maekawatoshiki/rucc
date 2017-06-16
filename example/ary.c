int printf(char *, ...);

int main() {
  int a[2];
  a[0] = 12;
  a[1] = 23;
  a[0] = a[0] + a[1];
  printf("%d %d%c", a[0], 0[a], 0xa);
}
