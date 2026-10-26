import 'dart:convert';
import 'dart:ffi';

String releasePlatform(Abi abi) => switch (abi) {
  Abi.linuxX64 => 'x86_64-linux',
  Abi.linuxArm64 => 'aarch64-linux',
  Abi.macosX64 => 'x86_64-darwin',
  Abi.macosArm64 => 'aarch64-darwin',
  _ => throw UnsupportedError(
    'falcon supports only Linux and macOS on x86_64 or arm64 (found $abi).',
  ),
};

String checksumFor(String sums, String asset) {
  final entry = RegExp(r'^([a-fA-F0-9]{64}) [ *](.+)$');
  for (final line in const LineSplitter().convert(sums)) {
    final match = entry.firstMatch(line);
    if (match != null && match[2] == asset) {
      return match[1]!.toLowerCase();
    }
  }
  throw FormatException('SHA256SUMS has no checksum for $asset.');
}

String releaseVersion(String value) {
  final normalized = value.startsWith('v') ? value.substring(1) : value;
  if (!RegExp(
    r'^\d+\.\d+\.\d+(?:-[0-9A-Za-z]+(?:[.-][0-9A-Za-z]+)*)?(?:\+[0-9A-Za-z]+(?:[.-][0-9A-Za-z]+)*)?$',
  ).hasMatch(normalized)) {
    throw FormatException('Invalid falcon release version: $value.');
  }
  return normalized;
}
