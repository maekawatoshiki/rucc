int printf(char *, ...);
int puts(char *);

struct S {
  char a, b, c;
  // int a, b, c;
  double f;
};

#define PRINT_SIZEOF(type) printf("sizeof(%s) = %d%c", #type, sizeof(type), 0xa)

int main() {
  char s[][8] = {"hello", "rucc", "world"}; 
  int i; double f;
  PRINT_SIZEOF( char                            );
  PRINT_SIZEOF( short                           );
  PRINT_SIZEOF( int                             );
  PRINT_SIZEOF( long                            );
  PRINT_SIZEOF( long long                       );
  PRINT_SIZEOF( float                           );
  PRINT_SIZEOF( double                          );
  PRINT_SIZEOF( int [5]                         );
  PRINT_SIZEOF( struct S                        );
  PRINT_SIZEOF( "a"                             );
  PRINT_SIZEOF( 1 + 2                           );
  PRINT_SIZEOF( i + f                           );
  PRINT_SIZEOF( s                               );
  PRINT_SIZEOF( s + i                           );
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
