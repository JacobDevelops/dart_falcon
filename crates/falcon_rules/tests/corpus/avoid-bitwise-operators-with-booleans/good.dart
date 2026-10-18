void example(bool left, bool right, int bits, dynamic unknown) {
  final a = left && right;
  final b = left || right;
  final c = bits & 3;
  final d = bits | 4;
  final e = left ^ right;
  final f = unknown & right;
  left &= right;
  left |= right;
}
