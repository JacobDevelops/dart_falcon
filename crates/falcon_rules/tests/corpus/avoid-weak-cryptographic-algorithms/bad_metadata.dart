import 'package:crypto/crypto.dart';
import 'package:crypto/crypto.dart' as crypto;

class Uses {
  const Uses(Object algorithm);
}

@Uses(md5) /* expect: avoid-weak-cryptographic-algorithms */
extension on List<int> {
  String get md5 => '';
  String useOwnGetter() => md5;
}

@Uses(crypto.sha1) /* expect: avoid-weak-cryptographic-algorithms */
extension AnnotatedPrefix on String {
  LocalHashes get crypto => LocalHashes();
  Object useOwnPrefix() => crypto.sha1;
}

class LocalHashes {
  final sha1 = 1;
}
