void bad() {
  items.forEach((e) { print(e); }); /* expect: avoid-function-literals-in-foreach-calls */
  list.forEach((x) => print(x)); /* expect: avoid-function-literals-in-foreach-calls */
  names.forEach((name) { save(name); }); /* expect: avoid-function-literals-in-foreach-calls */
  values.forEach((v) => sink(v)); /* expect: avoid-function-literals-in-foreach-calls */
  data.forEach((item) { process(item); log(item); }); /* expect: avoid-function-literals-in-foreach-calls */
}
