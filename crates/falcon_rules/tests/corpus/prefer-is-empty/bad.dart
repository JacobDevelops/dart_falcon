void bad() {
  if (list.length == 0) return; /* expect: prefer-is-empty */
  if (0 == items.length) return; /* expect: prefer-is-empty */
  if (str.length < 1) return; /* expect: prefer-is-empty */
  if (map.length <= 0) return; /* expect: prefer-is-empty */
  if (1 > set.length) return; /* expect: prefer-is-empty */
  if (0 >= names.length) return; /* expect: prefer-is-empty */
}
