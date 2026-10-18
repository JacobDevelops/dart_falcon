import 'package:crypto/crypto.dart';
import 'package:crypto/crypto.dart' as crypto;
import 'package:lookalike/lookalike.dart' as unrelated;

void strongAlgorithms(List<int> bytes) {
  final first = sha256;
  final second = crypto.sha512;
  Hmac(sha256, bytes);
  crypto.sha256.convert(bytes);
  final text = 'md5 sha1 DES DES3 RC4';
}

void shadowedHash(Object md5, Object sha1) {
  final first = md5;
  final second = sha1;
}

void shadowedPrefix(LocalHashes crypto) {
  final first = crypto.md5;
  final second = crypto.sha1;
}

class LocalHashes {
  final md5 = 1;
  final sha1 = 2;
  void useMembers() {
    final first = md5;
    final second = sha1;
  }
}

void unrelatedPackage() {
  final first = unrelated.md5;
  final second = unrelated.sha1;
  unrelated.DES();
  unrelated.DES3();
  unrelated.RC4();
}
