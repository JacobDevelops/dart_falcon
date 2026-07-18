void bad() {
  if (list.length != 0) return; /* expect: prefer-is-not-empty */
  if (0 != items.length) return; /* expect: prefer-is-not-empty */
  if (str.length > 0) return; /* expect: prefer-is-not-empty */
  if (0 < map.length) return; /* expect: prefer-is-not-empty */
  if (set.length >= 1) return; /* expect: prefer-is-not-empty */
  if (1 <= names.length) return; /* expect: prefer-is-not-empty */
}
