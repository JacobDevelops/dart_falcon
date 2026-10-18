import 'package:crypto/crypto.dart';
import 'package:crypto/crypto.dart' as crypto;
import 'package:dart_des/dart_des.dart';
import 'package:dart_des/dart_des.dart' as des;
import 'package:simple_rc4/simple_rc4.dart';
import 'package:simple_rc4/simple_rc4.dart' as stream;

void weakAlgorithms(List<int> bytes) {
  final first = md5; /* expect: avoid-weak-cryptographic-algorithms */
  final second = sha1; /* expect: avoid-weak-cryptographic-algorithms */
  crypto.md5.convert(bytes); /* expect: avoid-weak-cryptographic-algorithms */
  Hmac(crypto.sha1, bytes); /* expect: avoid-weak-cryptographic-algorithms */
  DES(key: bytes); /* expect: avoid-weak-cryptographic-algorithms */
  DES3(key: bytes); /* expect: avoid-weak-cryptographic-algorithms */
  RC4('secret'); /* expect: avoid-weak-cryptographic-algorithms */
  new des.DES(key: bytes); /* expect: avoid-weak-cryptographic-algorithms */
  new des.DES3(key: bytes); /* expect: avoid-weak-cryptographic-algorithms */
  new stream.RC4.fromBytes(bytes); /* expect: avoid-weak-cryptographic-algorithms */
  stream.RC4.fromBytes(bytes); /* expect: avoid-weak-cryptographic-algorithms */
  final constructor = stream.RC4.new; /* expect: avoid-weak-cryptographic-algorithms */
}
