class Hash {
  const Hash();
  List<int> convert(List<int> input) => input;
}
const md5 = Hash();
const sha1 = Hash();
const sha256 = Hash();
const sha512 = Hash();
class Hmac {
  Hmac(Hash hash, List<int> key);
}
