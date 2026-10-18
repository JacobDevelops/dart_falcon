void target({int alpha = 0, int zebra = 0}) {}
void calls() {
  target(zebra: 2, alpha: 1);
  target(alpha: 1, /* expect: arguments-ordering */ zebra: 2);
}
