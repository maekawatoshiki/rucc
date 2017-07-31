/*
 * Calculator 
 * - this code is written in C that rucc can process.
 */

#include <stdio.h>
#include <stdlib.h>
#include <ctype.h>
#include <string.h>

enum NODE_KIND {
  KIND_NUM,
  KIND_OP
};

char *input; 

typedef struct node_t {
  enum NODE_KIND kind;
  double num;
  struct {
    char op;
    struct node_t *left, *right;
  } op;
} node_t;

node_t *make_number(double n) {
  node_t *num = malloc(sizeof(node_t));
  num->kind = KIND_NUM;
  num->num = n;
  return num;
}
node_t *make_op(char op, node_t *left, node_t *right) {
  node_t *num = malloc(sizeof(node_t));
  num->kind = KIND_OP;
  num->op.op = op;
  num->op.left = left;
  num->op.right = right;
  return num;
}

node_t *expr_addsub();

node_t *expr_number() {
  if(*input == '(') {
    input++;
    node_t *n = expr_addsub();
    input++;
    return n;
  }
  if(!isdigit(*input)) return NULL; 
  char buf[16] = {0};
  for(int i = 0; isdigit(*input) || *input == '.'; i++) 
    buf[i] = *input++;
  return make_number(atof(buf));
}

node_t *expr_muldiv() {
  node_t *left = expr_number();
  while(*input == '*' || *input == '/') {
    char op = *input++;
    node_t *right = expr_number();
    left = make_op(op, left, right); 
  }
  return left;
}

node_t *expr_addsub() {
  node_t *left = expr_muldiv();
  while(*input == '+' || *input == '-') {
    char op = *input++;
    node_t *right = expr_muldiv();
    left = make_op(op, left, right); 
  }
  return left;
}

double calc(node_t *node) {
  if(node->kind == KIND_OP) {
    double l = calc(node->op.left), r = calc(node->op.right);
    // TODO: 'switch' not supported
    if(node->op.op == '+') return l + r;
    if(node->op.op == '-') return l - r;
    if(node->op.op == '*') return l * r;
    if(node->op.op == '/') return l / r;
  } else return node->num;
}

void show(node_t *node) {
  if(node->kind == KIND_OP) {
    printf("(%c ", node->op.op);
    show(node->op.left);
    show(node->op.right);
    printf(") ");
  } else printf("%.10g ", node->num);
}

int main(int argc, char *argv[]) {
  if(argc == 1) {
    puts("./calc [ an expression without spaces; e.g. 3/2+1.2*(2.1-3) ]");
    return 0;
  }

  input = argv[1];

  node_t *node = expr_addsub();
  show(node); puts("");

  printf("%.10g%c", calc(node), 10);
  return 0;
}
