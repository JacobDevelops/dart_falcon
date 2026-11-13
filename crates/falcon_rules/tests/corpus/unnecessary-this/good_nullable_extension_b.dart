extension NullableText on String? {
  int get size => this?.length ?? 0;
  String upper() => this?.toUpperCase() ?? '';
  int get forced => this!.length;
  void cascade() { this?..toUpperCase(); }
}
class Cascade {
  int value = 0;
  void update() { this..value = 1; }
}
