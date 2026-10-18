import 'package:crypto/crypto.dart';
import 'package:dart_des/dart_des.dart';
import 'package:simple_rc4/simple_rc4.dart';

const md5 = 1;
const sha1 = 2;
class DES {
  DES();
}
class DES3 {
  DES3();
}
class RC4 {
  RC4();
}

void localDeclarations() {
  final first = md5;
  final second = sha1;
  DES();
  new DES3();
  RC4();
}
