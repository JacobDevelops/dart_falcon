class GoodTraffic {
  const GoodTraffic._(this.value);
  final int value;

  static const GoodTraffic red = GoodTraffic._(0);
  static const GoodTraffic yellow = GoodTraffic._(1);
  static const GoodTraffic green = GoodTraffic._(2);
}

void exhaustive(GoodTraffic light) {
  switch (light) {
    case GoodTraffic.red:
      break;
    case GoodTraffic.yellow:
      break;
    case GoodTraffic.green:
      break;
  }
}

void withDefault(GoodTraffic light) {
  switch (light) {
    default:
      break;
  }
}

class PubliclyConstructible {
  const PubliclyConstructible();
  static const PubliclyConstructible one = PubliclyConstructible();
  static const PubliclyConstructible two = PubliclyConstructible();
}

void ignored(PubliclyConstructible value) {
  switch (value) {}
}
