class BooleanPrefixes {
  /// Boolean with is prefix
  bool isActive = true;

  /// Boolean with is prefix
  bool isValid = false;

  /// Boolean with is prefix
  bool isLoading = false;

  /// Boolean with has prefix
  bool hasPermission = true;

  /// Boolean with can prefix
  bool canEdit = false;

  /// Boolean with should prefix
  bool shouldRefresh = true;

  /// Boolean with was prefix
  bool wasSuccessful = true;

  /// Method parameter with is prefix
  void checkStatus(bool isReady) {
    print(isReady);
  }

  /// Method return with is prefix
  bool isStatusValid() {
    return true;
  }

  /// Local variable with is prefix
  void example() {
    bool isComplete = false;
    print(isComplete);
  }

  /// Late variable with has prefix
  late bool hasInitialized;

  /// Field with can prefix
  bool canUpdate = false;
}
