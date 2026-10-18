int useState(int initial) => initial;
int useMemoized(int Function() callback) => callback();

void useUnrelated(bool enabled) {
  if (enabled) useState(0);
  final value = enabled ? useState(1) : useState(2);
  for (var index = 0; index < value; index++) {
    useMemoized(() => index);
  }
}
