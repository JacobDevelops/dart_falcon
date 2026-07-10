/// Validates tokens, e.g. JWT and opaque tokens. /* expect: avoid_abbreviations_in_doc_comments */
class AuthService {
  /// Rate limiting, i.e. throttling requests. /* expect: avoid_abbreviations_in_doc_comments */
  bool validateToken(String token) => token.isNotEmpty;

  /// Handles sessions, cookies, etc. /* expect: avoid_abbreviations_in_doc_comments */
  late Map<String, dynamic> config;

  /// See Smith et al. for the derivation. /* expect: avoid_abbreviations_in_doc_comments */
  void checkParams(String param) {
    print(param);
  }

  /// Two on one line e.g. this and i.e. that. /* expect: avoid_abbreviations_in_doc_comments */ /* expect: avoid_abbreviations_in_doc_comments */
  void init() {}
}
