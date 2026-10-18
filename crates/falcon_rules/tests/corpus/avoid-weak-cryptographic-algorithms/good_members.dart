import 'package:crypto/crypto.dart';
import 'package:crypto/crypto.dart' as crypto;

extension NamedHashMembers on List<int> {
  String get md5 => '';
  String sha1() => '';
  String useGetter() => md5;
  String useMethod() => sha1();
}

extension on List<int> {
  String get md5 => '';
  static String sha1() => '';
  String useGetter() => md5;
  String useStaticMethod() => sha1();
}

extension NamedPrefixMember on String {
  Hashes get crypto => Hashes();
  Object usePrefix() => crypto.md5;
}

extension on int {
  Hashes crypto() => Hashes();
  Object usePrefixMethod() => crypto().sha1;
}

class Hashes {
  String get md5 => '';
  String sha1() => '';
  Object useGetter() => md5;
  Object useMethod() => sha1();
}

class StaticHashes {
  static String get md5 => '';
  static String sha1() => '';
  Object useGetter() => md5;
  Object useMethod() => sha1();
}

class PrefixMember {
  final crypto = Hashes();
  Object usePrefix() => crypto.md5;
}
