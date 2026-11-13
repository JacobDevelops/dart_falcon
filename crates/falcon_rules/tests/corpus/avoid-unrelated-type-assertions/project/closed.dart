final class Closed {}
base mixin M implements Closed {}
abstract interface class Right {}
sealed class Sealed {}
abstract interface class Box<T> {}
final class G<T> extends Sealed implements Box<T> {}
