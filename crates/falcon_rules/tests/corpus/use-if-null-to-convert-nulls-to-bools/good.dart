void check(bool? flag, bool strict, Object? other) {
  if (flag ?? false) {}
  if (flag ?? true) {}
  print(flag == false);
  print(flag != true);
  print(strict == true);
  print(true == flag);
  print(other == true);
  print(flag == null);
}

void promoted(bool? early) {
  if (early == null) return;
  if (early == true) print(1);
}
