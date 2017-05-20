int printf(char *, ...);

int fact(int n) {
  if(n < 2) return 1;
  else return fact(n - 1) * n;
}

int main(int argc, char *argv[]) {
  int i = 1;
  while(i < 10) {
    printf("%d! = %d%c", i, fact(i), 0xa);
    i = i + 1;
  }
}
