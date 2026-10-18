void target({int zebra = 0, int alpha = 0}) {}
void calls() {
  target(alpha: 1, zebra: 2);
  target(zebra: 2, /* expect: arguments-ordering */ alpha: 1);
}
