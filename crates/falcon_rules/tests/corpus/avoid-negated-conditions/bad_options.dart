void f(bool flag) {
  if (!flag) {} else {} /* expect: avoid-negated-conditions */
}
