int printf(char *, ...);
int puts(char *);

int add1(int n) {
  return n + 1;
}

// all qualifiers (for coverage)
typedef int I;
static I i1;
I f1() { auto I i2; register I i3; return 0; }
const I i4 = 1;
volatile I i5;
inline I f2() { I * restrict i6 = 0; return 0; }

int main(int argc, char *argv[]) {
  printf("add1(2) = %d%c", add1(2), 0xa);
  printf("hello" 
         " " 
         "world%s", 
         "!!");
  return 0;
}
