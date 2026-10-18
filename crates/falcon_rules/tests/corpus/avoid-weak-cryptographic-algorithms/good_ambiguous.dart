import 'package:crypto/crypto.dart';
import 'package:lookalike/lookalike.dart';
import 'package:crypto/crypto.dart' as combined;
import 'package:lookalike/lookalike.dart' as combined;

void ambiguousImports() {
  final first = md5;
  final second = sha1;
  final prefixedFirst = combined.md5;
  final prefixedSecond = combined.sha1;
}
