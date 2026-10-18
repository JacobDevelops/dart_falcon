import 'package:crypto/crypto.dart' show sha256;
import 'package:crypto/crypto.dart' as crypto hide md5, sha1;
import 'exports.dart' as exported show sha256;

void safeImports(List<int> bytes) {
  sha256.convert(bytes);
  crypto.sha256.convert(bytes);
  exported.sha256.convert(bytes);
}
