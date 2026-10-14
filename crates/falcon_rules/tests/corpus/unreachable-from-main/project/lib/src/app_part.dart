part of '../app.dart';

class PartWidget {
  void render() {
    print(OwnerUsedFromPart);
  }

  void unusedRender() {} /* expect: unreachable-from-main */
}

/// Delegates to [ownerFunctionUsedFromPart].
void helperInPart() {}

class PartUnused {} /* expect: unreachable-from-main */
