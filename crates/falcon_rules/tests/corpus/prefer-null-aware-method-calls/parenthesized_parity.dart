// Analyzer reports no lint for adv-AdvB4/repro/lib/pnamc_parens.dart.
void f(void Function()? fn) {
  if (fn != null) (fn!)();
  var x = fn != null ? (fn!()) : null;
}
