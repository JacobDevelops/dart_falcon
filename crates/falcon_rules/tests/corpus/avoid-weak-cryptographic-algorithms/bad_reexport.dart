import 'exports.dart' as algorithms;

void reexportedAlgorithms(List<int> bytes) {
  final hash = algorithms.md5; /* expect: avoid-weak-cryptographic-algorithms */
  final hash2 = algorithms.sha1; /* expect: avoid-weak-cryptographic-algorithms */
  algorithms.DES(key: bytes); /* expect: avoid-weak-cryptographic-algorithms */
  algorithms.DES3(key: bytes); /* expect: avoid-weak-cryptographic-algorithms */
  algorithms.RC4('secret'); /* expect: avoid-weak-cryptographic-algorithms */
}
