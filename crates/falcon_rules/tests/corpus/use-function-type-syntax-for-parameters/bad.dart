// Old-style function-typed parameters.
void forEach(int f(int x)) {} /* expect: use-function-type-syntax-for-parameters */
void run(void cb()) {} /* expect: use-function-type-syntax-for-parameters */
int apply(int g(int a, int b)) => g(1, 2); /* expect: use-function-type-syntax-for-parameters */
void sortBy(bool compare(int a, int b)) {} /* expect: use-function-type-syntax-for-parameters */
void schedule(void task(), int delay) {} /* expect: use-function-type-syntax-for-parameters */

class C {
  void method(String fn(int x)) {} /* expect: use-function-type-syntax-for-parameters */
}
