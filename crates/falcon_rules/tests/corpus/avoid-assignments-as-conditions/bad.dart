class Box {
  bool value = false;
}

bool identity(bool value) => value;

void conditions(bool input, bool other, bool? nullable, Box box) {
  var flag = false;
  var count = 0;
  if (flag = input) {} /* expect: avoid-assignments-as-conditions */
  while (flag = input) { break; } /* expect: avoid-assignments-as-conditions */
  do { count++; } while (flag = input); /* expect: avoid-assignments-as-conditions */
  for (; flag = input;) { break; } /* expect: avoid-assignments-as-conditions */
  final answer = (flag = input) ? 1 : 2; /* expect: avoid-assignments-as-conditions */
  final list = [if (flag = input) answer]; /* expect: avoid-assignments-as-conditions */
  final set = {if (flag = input) answer}; /* expect: avoid-assignments-as-conditions */
  final map = {if (flag = input) 'answer': answer}; /* expect: avoid-assignments-as-conditions */
  if ((nullable ??= input) && other) {} /* expect: avoid-assignments-as-conditions */
  if (identity(flag = other)) {} /* expect: avoid-assignments-as-conditions */
  if ((count += 1) > 2) {} /* expect: avoid-assignments-as-conditions */
  final generated = [for (; flag = input;) answer]; /* expect: avoid-assignments-as-conditions */
  final keyed = {for (; flag = input;) 'answer': answer}; /* expect: avoid-assignments-as-conditions */
  if (input ? (flag = other) : false) {} /* expect: avoid-assignments-as-conditions */
  if ((box..value = input).value) {} /* expect: avoid-assignments-as-conditions */
  final values = [false];
  if ((values..[0] = input).first) {} /* expect: avoid-assignments-as-conditions */
  if (() {
    if (flag = input) {} /* expect: avoid-assignments-as-conditions */
    return true;
  }()) {}
  if (input case true when (flag = other)) {} /* expect: avoid-assignments-as-conditions */
  switch (input) {
    case true when (flag = other): /* expect: avoid-assignments-as-conditions */
      break;
    default:
      break;
  }
  final switched = switch (input) {
    true when (flag = other) => 1, /* expect: avoid-assignments-as-conditions */
    _ => 0,
  };
}
