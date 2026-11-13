void checks(List<int> xs) {
  if (xs.length == 0) return; /* expect: prefer-is-empty */
  if (0 == xs.length) return; /* expect: prefer-is-empty */
  if (xs.length != 0) return; /* expect: prefer-is-empty */
  if (0 != xs.length) return; /* expect: prefer-is-empty */
  if (xs.length < 0) return; /* expect: prefer-is-empty */
  if (0 > xs.length) return; /* expect: prefer-is-empty */
  if (xs.length <= 0) return; /* expect: prefer-is-empty */
  if (0 >= xs.length) return; /* expect: prefer-is-empty */
  if (xs.length > 0) return; /* expect: prefer-is-empty */
  if (0 < xs.length) return; /* expect: prefer-is-empty */
  if (xs.length >= 0) return; /* expect: prefer-is-empty */
  if (0 <= xs.length) return; /* expect: prefer-is-empty */
  if (xs.length < 1) return; /* expect: prefer-is-empty */
  if (1 > xs.length) return; /* expect: prefer-is-empty */
  if (xs.length >= 1) return; /* expect: prefer-is-empty */
  if (1 <= xs.length) return; /* expect: prefer-is-empty */
}
