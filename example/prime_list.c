int printf(char *, ...);
int puts(char *);
int atoi(char *);

int is_prime(int n) {
  if(n == 2) return 1;
  if(n % 2 == 0) return 0;
  for(int i = 3; i * i <= n; i = i + 2) {
    if(n % i == 0) return 0;
  }
  return 1;
}

int main(int argc, char *argv[]) {
  if(argc == 1) {
    puts("./prime_list [max_num(>2)]");
    return 0;
  }

  int max_num = atoi(argv[1]);
  if(max_num <= 2) { puts("max_num must be greater than 2"); return 0; }

  puts("2");
  for(int i = 3; i <= max_num; i = i + 2) {
    if(is_prime(i)) { printf("%d%c", i, 0xa); }
  }
}
