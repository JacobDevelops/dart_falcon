// Named extensions must be UpperCamelCase.

extension my_ext on int {} /* expect: camel-case-extensions */

extension foo_bar on String {} /* expect: camel-case-extensions */

extension lowercase on double {} /* expect: camel-case-extensions */

extension mixedCase on num {} /* expect: camel-case-extensions */

extension a on bool {} /* expect: camel-case-extensions */

extension _lower on List {} /* expect: camel-case-extensions */
