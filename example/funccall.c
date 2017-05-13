int printf(char *, ...);
int puts(char *);

int add1(int n) {
  return n + 1;
}

int main(int argc, char *argv[]) {
  printf("add1(2) = %d%c", add1(2), 0xa);
  printf("hello world%s", "!!");
  return 0;
}
