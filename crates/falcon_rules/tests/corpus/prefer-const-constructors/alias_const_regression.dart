typedef Enabled = bool;
final bool enabled = Enabled.fromEnvironment('feature'); /* expect: prefer-const-constructors */
class Box {
  const Box();
  const Box.named();
}
typedef Alias = Box;
final unnamed = Alias(); /* expect: prefer-const-constructors */
final named = Alias.named(); /* expect: prefer-const-constructors */
final explicitNew = new Alias.named(); /* expect: prefer-const-constructors */
