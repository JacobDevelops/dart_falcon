/// Validates tokens, e.g. JWT and opaque tokens. /* expect: avoid-abbreviations-in-doc-comments */
class AuthService {
  /// Rate limiting, i.e. throttling requests. /* expect: avoid-abbreviations-in-doc-comments */
  bool validateToken(String token) => token.isNotEmpty;

  /// Handles sessions, cookies, etc. /* expect: avoid-abbreviations-in-doc-comments */
  late Map<String, dynamic> config;

  /// See Smith et al. for the derivation. /* expect: avoid-abbreviations-in-doc-comments */
  void checkParams(String param) {
    print(param);
  }

  /// Two on one line e.g. this and i.e. that. /* expect: avoid-abbreviations-in-doc-comments */ /* expect: avoid-abbreviations-in-doc-comments */
  void init() {}
}
