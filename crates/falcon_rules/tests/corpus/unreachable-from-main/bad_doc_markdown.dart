/// A real reference to [Documented].
/// A [Linked](https://example.com) label is not a Dartdoc reference.
/// Inline code `[InlineCode]` is not a reference either.
/// ```dart
/// [FencedCode]
/// ```
///
///     [IndentedCode]
///
/// [ReferenceLabel] [Destination]
/// [Definition]: https://example.com
/// {@template [TemplateDirective]}
/// @docImport [ImportDirective]
/// @nodoc [NoDocDirective]
void main() { alternateEntryPoint(); }

class Documented {}
class Linked {} /* expect: unreachable-from-main */
class InlineCode {} /* expect: unreachable-from-main */
class FencedCode {} /* expect: unreachable-from-main */
class IndentedCode {} /* expect: unreachable-from-main */
class ReferenceLabel {} /* expect: unreachable-from-main */
// The shared parser skips the label but retains the second bracket as a reference.
class Destination {}
class Definition {} /* expect: unreachable-from-main */
class TemplateDirective {} /* expect: unreachable-from-main */
class ImportDirective {} /* expect: unreachable-from-main */
class NoDocDirective {} /* expect: unreachable-from-main */

/// The shared documentation grouping discards this preceding [ReplacedDoc].
/** A block comment replaces the preceding line documentation. */
void alternateEntryPoint() {}
class ReplacedDoc {} /* expect: unreachable-from-main */
