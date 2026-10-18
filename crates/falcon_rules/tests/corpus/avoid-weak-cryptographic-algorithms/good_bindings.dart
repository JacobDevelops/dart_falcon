import 'package:crypto/crypto.dart' as crypto;
import 'package:dart_des/dart_des.dart';
import 'package:simple_rc4/simple_rc4.dart';

void localConstructors(Object Function() DES, Object Function() DES3,
    Object Function() RC4) {
  DES();
  DES3();
  RC4();
}

void localFunction() {
  Object DES() => Object();
  DES();
}

void typeParameter<DES>(DES value) {
  final type = DES;
}

class MemberPrefix {
  final crypto = LocalHashes();
  void usePrefix() {
    final first = crypto.md5;
    final second = crypto.sha1;
  }
}

class LocalHashes {
  final md5 = 1;
  final sha1 = 2;
}

void annotationsOnly(DES value, DES3 other, RC4 stream) {}
