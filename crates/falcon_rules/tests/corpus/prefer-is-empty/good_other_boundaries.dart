void checks(List<int> xs, List<int>? nullable) {
  if (xs.length <= 1) return;
  if (1 >= xs.length) return;
  if (xs.length > 1) return;
  if (1 < xs.length) return;
  if (nullable?.length == 0) return;
}
