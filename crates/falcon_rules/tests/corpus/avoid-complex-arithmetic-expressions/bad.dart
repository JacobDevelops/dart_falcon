int compute(int a, int b, int c, int d) => a + b * (c - d); /* expect: avoid-complex-arithmetic-expressions */
int calls(int a) => a + helper(a * a * a * a); /* expect: avoid-complex-arithmetic-expressions */
int helper(int x) => x;
double doubles(double a, double b, double c, double d) => a / b + c * d; /* expect: avoid-complex-arithmetic-expressions */
num numbers(num a, num b, num c, num d) => a + b * (c % d); /* expect: avoid-complex-arithmetic-expressions */
