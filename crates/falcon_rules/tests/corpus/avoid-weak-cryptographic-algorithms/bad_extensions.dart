import 'package:crypto/crypto.dart';
import 'package:crypto/crypto.dart' as crypto;

class Holder {
  final md5 = 1;
  final sha1 = 2;
  final crypto = 3;
}

extension NamedHashes on Holder {
  Object weakNamed(List<int> bytes) => md5.convert(bytes); /* expect: avoid-weak-cryptographic-algorithms */
  Object weakPrefixed(List<int> bytes) => crypto.sha1.convert(bytes); /* expect: avoid-weak-cryptographic-algorithms */
}

extension on Holder {
  Object weakUnnamed(List<int> bytes) => sha1.convert(bytes); /* expect: avoid-weak-cryptographic-algorithms */
  Object weakUnnamedPrefixed(List<int> bytes) => crypto.md5.convert(bytes); /* expect: avoid-weak-cryptographic-algorithms */
}
