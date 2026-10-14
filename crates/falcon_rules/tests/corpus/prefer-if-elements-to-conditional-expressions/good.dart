List<int> lists(bool flag, int a, int b) {
  return [
    1,
    if (flag) a else b,
  ];
}

int pick(bool flag, int a, int b) => flag ? a : b;

List<int> nestedArgument(bool flag, int a, int b) => [pick(flag, a, b), (flag ? a : b) + 1];

Map<String, int> maps(bool flag) => {'k': flag ? 1 : 2};

List<int> spread(bool flag, List<int> a, List<int> b) => [...flag ? a : b];

List<int> nullAware(bool flag, int? a) => [?a];

void call(bool flag) => print([1, 2].map((v) => flag ? v : -v));
