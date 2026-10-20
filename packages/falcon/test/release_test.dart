import 'dart:ffi';

import 'package:falcon/src/release.dart';
import 'package:test/test.dart';

void main() {
  test('maps supported ABIs to release platforms', () {
    expect(releasePlatform(Abi.linuxX64), 'x86_64-linux');
    expect(releasePlatform(Abi.linuxArm64), 'aarch64-linux');
    expect(releasePlatform(Abi.macosX64), 'x86_64-darwin');
    expect(releasePlatform(Abi.macosArm64), 'aarch64-darwin');
  });

  test('rejects unsupported operating systems and architectures', () {
    for (final abi in [Abi.windowsX64, Abi.linuxArm, Abi.androidArm64]) {
      expect(() => releasePlatform(abi), throwsUnsupportedError);
    }
  });

  test('looks up the exact archive checksum in sha256sum output', () {
    final first = 'a' * 64;
    final second = 'B' * 64;
    final sums = '$first  other.tar.gz\r\n$second *falcon.tar.gz\r\n';
    expect(checksumFor(sums, 'falcon.tar.gz'), second.toLowerCase());
    expect(checksumFor(sums, 'other.tar.gz'), first);
  });

  test('rejects a missing or malformed checksum entry', () {
    expect(
      () => checksumFor('${'a' * 64}  other.tar.gz\n', 'falcon.tar.gz'),
      throwsFormatException,
    );
    expect(
      () => checksumFor('invalid  falcon.tar.gz\n', 'falcon.tar.gz'),
      throwsFormatException,
    );
  });
}
