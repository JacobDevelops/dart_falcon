void f(bool flag) {
  if (!flag) {} else {} /* expect: avoid-negated-conditions */
  final list = [if (!flag) 1 else 2]; /* expect: avoid-negated-conditions */
  final map = {if (!flag) 1: 2 else 3: 4}; /* expect: avoid-negated-conditions */
}
