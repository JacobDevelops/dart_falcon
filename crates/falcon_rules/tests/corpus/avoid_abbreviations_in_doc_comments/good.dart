/// The implementation of the authentication service.
class AuthService {
  /// Helper function to validate tokens.
  bool validateToken(String token) => token.isNotEmpty;

  /// Stores configuration data in the repository.
  late Map<String, dynamic> config;

  /// Check parameter values, for example email validation.
  void checkParams(String param) {
    print(param);
  }

  /// Initialize utility variables and, that is, defaults.
  void init() {}

  // A regular (non-doc) comment is out of scope, e.g. this is fine.
  void process(String arg) {}

  /// Case-sensitive: E.g. and I.e. at sentence start are not flagged.
  void setup() {}
}
