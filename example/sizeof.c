int printf(char *, ...);
int puts(char *);

struct S {
  char a, b, c;
  // int a, b, c;
  double f;
};

#define PRINT_SIZEOF(type) printf("sizeof(%s) = %d%c", #type, sizeof(type), 0xa)

int main() {
  PRINT_SIZEOF(char);
  PRINT_SIZEOF(short);
  PRINT_SIZEOF(int);
  PRINT_SIZEOF(long);
  PRINT_SIZEOF(long long);
  PRINT_SIZEOF(float);
  PRINT_SIZEOF(double);
  PRINT_SIZEOF(int [5]);
  PRINT_SIZEOF(struct S);
}
