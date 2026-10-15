void bad(int x) {
  if (true) {} /* expect: literal-only-boolean-expressions */
  if (1 > 2) {} /* expect: literal-only-boolean-expressions */
  if (!false) {} /* expect: literal-only-boolean-expressions */
  if ('a' == 'b' || null == null) {} /* expect: literal-only-boolean-expressions */
  while (true && false) {} /* expect: literal-only-boolean-expressions */
  do {} while (false); /* expect: literal-only-boolean-expressions */
  for (var i = 0; 1 < 2; i++) {} /* expect: literal-only-boolean-expressions */
  if (1 is int) {} /* expect: literal-only-boolean-expressions */
  if (null ?? x > 1) {} /* expect: literal-only-boolean-expressions */
  switch (x) {
    case 1 when true: /* expect: literal-only-boolean-expressions */
      print(x);
    default:
  }
  final y = switch (x) {
    1 when 2 > 1 => 'a', /* expect: literal-only-boolean-expressions */
    _ => 'b',
  };
  print(y);
}

void constantInterpolation() {
  const a = 20;
  if ('$a' == '20') {} /* expect: literal-only-boolean-expressions */
  if ('${1 + 2}' == '3') {} /* expect: literal-only-boolean-expressions */
  const flag = true;
  if ('${flag ? a : 0}' == '20') {} /* expect: literal-only-boolean-expressions */
}
