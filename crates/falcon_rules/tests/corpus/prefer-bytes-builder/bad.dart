import 'dart:typed_data';

void compose(List<int> bytes, List<int> chunk, Uint8List typed) {
  bytes = bytes + chunk; /* expect: prefer-bytes-builder */
  bytes += chunk; /* expect: prefer-bytes-builder */
  bytes.addAll(chunk); /* expect: prefer-bytes-builder */
  bytes..addAll(chunk); /* expect: prefer-bytes-builder */
  final copied = [...bytes]; /* expect: prefer-bytes-builder */
  final nested = [if (true) ...chunk]; /* expect: prefer-bytes-builder */
  final loop = [for (final value in <List<int>>[chunk]) ...value]; /* expect: prefer-bytes-builder */
  final literals = [...<int>[1, 2]]; /* expect: prefer-bytes-builder */
  typed + chunk; /* expect: prefer-bytes-builder */
  final typedCopy = [...typed]; /* expect: prefer-bytes-builder */
  for (var i = 0; i < 3; i++) {
    bytes += typed; /* expect: prefer-bytes-builder */
  }
}

class Holder {
  List<int> bytes = <int>[];
}

void cascades(Holder holder, List<int> chunk, List<int>? maybe) {
  holder..bytes.addAll(chunk); /* expect: prefer-bytes-builder */
  holder..bytes += chunk; /* expect: prefer-bytes-builder */
  maybe?.addAll(chunk); /* expect: prefer-bytes-builder */
  final optional = [...?maybe]; /* expect: prefer-bytes-builder */
}

void chains(List<int> a, List<int> b, List<int> c, List<int> d) {
  // Each whole chain has exactly one diagnostic, including grouped operands.
  final linear = a + b + c; /* expect: prefer-bytes-builder */
  final grouped = (a + b) + (c + d); /* expect: prefer-bytes-builder */
  final rightGrouped = a + (b + (c + d)); /* expect: prefer-bytes-builder */
  final multiline =
      a + /* expect: prefer-bytes-builder */
      b +
      c;
  final multilineGrouped = ( /* expect: prefer-bytes-builder */
    a + b
  ) + (
    c + d
  );

  // Only the spread is reported, not its parenthesized concatenation.
  final spread = [...(a + b)]; /* expect: prefer-bytes-builder */
  final nestedParens = [...(((a + b) + c))]; /* expect: prefer-bytes-builder */
  final multilineSpread = [
    ...(
      a + b + c /* expect: prefer-bytes-builder */
    ),
  ];
  final directSpread = [...a + b + c]; /* expect: prefer-bytes-builder */

  // Separate compositions must retain separate diagnostics.
  final separate = [a + b, c + d]; /* expect: prefer-bytes-builder */ /* expect: prefer-bytes-builder */
  final siblingSpreads = [...(a + b), ...(c + d)]; /* expect: prefer-bytes-builder */ /* expect: prefer-bytes-builder */
  final argument = a + identity(b + c); /* expect: prefer-bytes-builder */ /* expect: prefer-bytes-builder */
}

List<int> identity(List<int> bytes) => bytes;
