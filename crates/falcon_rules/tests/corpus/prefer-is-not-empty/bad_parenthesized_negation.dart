void checks(List<int> xs) {
  if (!(xs.isEmpty)) return; /* expect: prefer-is-not-empty */
  if (!((xs.isEmpty))) return; /* expect: prefer-is-not-empty */
}
