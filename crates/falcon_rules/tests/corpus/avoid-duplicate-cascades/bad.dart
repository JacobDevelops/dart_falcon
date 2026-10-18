void f(dynamic value) {
  value..name = 'first'..name = 'second'; /* expect: avoid-duplicate-cascades */
  value..count = 1..count = 1; /* expect: avoid-duplicate-cascades */
  value..[0] = 1..[0] = 2; /* expect: avoid-duplicate-cascades */
  value..child.name = 'a'..child.name = 'b'; /* expect: avoid-duplicate-cascades */
  value..name = 'a'..other = 'b'..name = 'c'; /* expect: avoid-duplicate-cascades */
  value..[1 + 2] = 0..[1+2] = 1; /* expect: avoid-duplicate-cascades */
}
