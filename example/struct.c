int printf(char *, ...);
int puts(char *);

struct A {
  int a, b, c;
};

typedef struct {
  char *name;
  int age;
} User;

int main() {
  struct A a;
  a.a = a.b = 1;

  User u;
  u.name = "uint256_t";
  u.age = 16;
  printf("%s %d%c", u.name, u.age, 0xa);
}
