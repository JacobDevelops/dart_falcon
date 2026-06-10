/// The impl of the auth service /* expect: avoid_abbreviations_in_doc_comments */
class AuthService {
  /// Helper func to validate tokens /* expect: avoid_abbreviations_in_doc_comments */
  bool validateToken(String token) => token.isNotEmpty;

  /// Stores config data in the repo /* expect: avoid_abbreviations_in_doc_comments */
  late Map<String, dynamic> config;

  /// Check param values e.g. email validation /* expect: avoid_abbreviations_in_doc_comments */
  void checkParams(String param) {
    print(param);
  }

  /// Initialize util vars and i.e. defaults /* expect: avoid_abbreviations_in_doc_comments */
  void init() {}

  /// Process arg and cfg settings etc. /* expect: avoid_abbreviations_in_doc_comments */
  void process(String arg) {}

  /// Handle var initialization and setup /* expect: avoid_abbreviations_in_doc_comments */
  void setup() {}
}
