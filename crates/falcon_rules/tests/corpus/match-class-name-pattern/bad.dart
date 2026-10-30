class First {} /* expect: match-class-name-pattern */
abstract class Second {} /* expect: match-class-name-pattern */
final class Third {} /* expect: match-class-name-pattern */
sealed class Fourth {} /* expect: match-class-name-pattern */
class Fifth<T> {} /* expect: match-class-name-pattern */
class ValidState {}
class _Private {}
mixin class _Mixin {}
class Alias = Object with _Mixin; /* expect: match-class-name-pattern */
mixin class Reusable {} /* expect: match-class-name-pattern */
