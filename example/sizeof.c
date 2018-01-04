int printf(char *, ...);
int puts(char *);

struct S {
  char a, b, c;
  // int a, b, c;
  double f;
};

union U {
  struct S s;
  char hello[6];
};

enum { ZERO, ONE };

#define PRINT_SIZEOF(type) printf("sizeof(%s) = %d%c", #type, sizeof(type), 0xa)

int main() {
  char str[][8] = {"hello", "rucc", "world"}; 
  int i; double f; struct S s; union U u;
  PRINT_SIZEOF( char                            );
  PRINT_SIZEOF( short                           );
  PRINT_SIZEOF( int                             );
  PRINT_SIZEOF( long                            );
  PRINT_SIZEOF( long long                       );
  PRINT_SIZEOF( float                           );
  PRINT_SIZEOF( double                          );
  PRINT_SIZEOF( int [5]                         );
  PRINT_SIZEOF( struct S                        );
  PRINT_SIZEOF( union U                         );
  PRINT_SIZEOF( u                               );
  PRINT_SIZEOF( s.f                             );
  PRINT_SIZEOF( "a"                             );
  PRINT_SIZEOF( 1 + 2                           );
  PRINT_SIZEOF( 1.2 + 3.4                       );
  PRINT_SIZEOF( 100000000000 + 200000000000     );
  PRINT_SIZEOF( ZERO + ONE                      );
  PRINT_SIZEOF( i + f                           );
  PRINT_SIZEOF( str                             );
  PRINT_SIZEOF( str + i                         );
  PRINT_SIZEOF( main                            );
  PRINT_SIZEOF( main + 8                        );
  PRINT_SIZEOF( puts("hello")                   );
  PRINT_SIZEOF( !1                              );
  PRINT_SIZEOF( ~1                              );
  PRINT_SIZEOF( &i                              );
  PRINT_SIZEOF( i++                             );
  PRINT_SIZEOF( i--                             );
  PRINT_SIZEOF( *&f                             );
  PRINT_SIZEOF( 1 ? 100000000000 : 200000000000 );
}
