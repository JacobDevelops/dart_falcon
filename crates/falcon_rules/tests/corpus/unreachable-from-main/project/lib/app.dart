part 'src/app_part.dart';

void main() {
  PartWidget().render();
  helperInPart();
}

class OwnerUnused {} /* expect: unreachable-from-main */

class OwnerUsedFromPart {}

void ownerFunctionUsedFromPart() {}
