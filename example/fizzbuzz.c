int printf(char *, ...);
int puts(char *);
int atoi(char *);

int main(int argc, char *argv[]) {
  if(argc == 1) {
    puts("./fizzbuzz [max_num]");
    return 0;
  }

  const int max_num = atoi(argv[1]);
  for(int i = 1; i <= max_num; i = i + 1) {
    if(i % 15 == 0) {
      puts("fizzbuzz");
    } else if(i % 5 == 0) {
      puts("buzz");
    } else if(i % 3 == 0) {
      puts("fizz");
    } else {
      printf("%d%c", i, 0xa);
    }
  }
}
