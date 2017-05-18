int printf(char *, ...);
int puts(char *);
int atoi(const char *);



int fact(int n) {
  if(n == 1)
    return 1;
  else 
    return fact(n - 1) * n;
}

int main(int argc, char *argv[]) {
  if(argc < 2) {
    puts("./fact [NUMBER]");
    return 0;
  } 

  int n = atoi(argv[1]);
  printf("fact(%d) = %d%c", n, fact(n), 0xA);
}
