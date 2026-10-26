import 'dart:convert';
import 'dart:ffi';
import 'dart:io';

import 'package:crypto/crypto.dart';
import 'package:falcon/src/release.dart';
import 'package:test/test.dart';

import '../bin/falcon.dart' as launcher;

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

  group('launcher updates', () {
    late Directory cache;
    late HttpServer server;
    late List<String> requests;
    late List<int> archive;
    var corruptChecksum = false;
    const platform = 'x86_64-linux';

    setUp(() async {
      cache = await Directory.systemTemp.createTemp('falcon-update-test-');
      final fixture = await Directory('${cache.path}/fixture').create();
      await File('${fixture.path}/falcon').writeAsString('new binary');
      final tar = await Process.run('tar', [
        '-czf',
        '${fixture.path}/release.tar.gz',
        '-C',
        fixture.path,
        'falcon',
      ]);
      expect(tar.exitCode, 0, reason: '${tar.stderr}');
      archive = await File('${fixture.path}/release.tar.gz').readAsBytes();
      requests = [];
      corruptChecksum = false;
      server = await HttpServer.bind(InternetAddress.loopbackIPv4, 0);
      server.listen((request) async {
        final path = request.uri.path;
        requests.add(path);
        if (path == '/latest') {
          request.response.write(jsonEncode({'tag_name': 'v2.3.4'}));
        } else if (path.endsWith('/SHA256SUMS')) {
          final selected = path.split('/')[3].substring(1);
          final digest = corruptChecksum ? '0' * 64 : sha256.convert(archive);
          request.response.write(
            '$digest  falcon-$selected-$platform.tar.gz\n',
          );
        } else if (path.endsWith('.tar.gz')) {
          request.response.add(archive);
        } else {
          request.response.statusCode = HttpStatus.notFound;
        }
        await request.response.close();
      });
    });

    tearDown(() async {
      await server.close(force: true);
      await cache.delete(recursive: true);
    });

    Future<String> update({String? requested, String? pinned}) {
      final base = 'http://${server.address.host}:${server.port}';
      return launcher.selfUpdate(
        cache,
        platform,
        requestedVersion: requested,
        pinnedVersion: pinned,
        releasesUrl: '$base/releases',
        latestReleaseUrl: '$base/latest',
      );
    }

    test('updates to latest and persists the launch selection', () async {
      final installed = File('${cache.path}/2.3.4/falcon');
      await installed.parent.create();
      await installed.writeAsString('old binary');
      expect(await update(), '2.3.4');
      expect(
        await File('${cache.path}/2.3.4/falcon').readAsString(),
        'new binary',
      );
      expect(await launcher.selectedVersion(cache), '2.3.4');
      expect(requests, contains('/latest'));
      expect(
        requests,
        contains('/releases/download/v2.3.4/falcon-2.3.4-$platform.tar.gz'),
      );
    });

    test(
      'checksum mismatch preserves installed binary and selection',
      () async {
        final directory = await Directory('${cache.path}/2.3.4').create();
        final binary = File('${directory.path}/falcon');
        await binary.writeAsString('installed binary');
        await File('${cache.path}/active-version').writeAsString('1.2.3');
        corruptChecksum = true;

        await expectLater(update(requested: '2.3.4'), throwsStateError);

        expect(await binary.readAsString(), 'installed binary');
        expect(await launcher.selectedVersion(cache), '1.2.3');
        expect(await directory.list().map((entry) => entry.path).toList(), [
          binary.path,
        ]);
      },
    );

    test(
      'environment pin bypasses latest and overrides saved selection',
      () async {
        await File('${cache.path}/active-version').writeAsString('2.3.4');
        expect(await update(pinned: 'v1.2.3'), '1.2.3');
        expect(await launcher.selectedVersion(cache), '1.2.3');
        expect(requests, isNot(contains('/latest')));
        expect(
          requests,
          contains('/releases/download/v1.2.3/falcon-1.2.3-$platform.tar.gz'),
        );
        expect(
          await launcher.selectedVersion(cache, pinnedVersion: 'v1.2.3'),
          '1.2.3',
        );
      },
    );

    test(
      'explicit update version takes precedence over environment pin',
      () async {
        expect(await update(requested: 'v3.4.5', pinned: '1.2.3'), '3.4.5');
        expect(requests, isNot(contains('/latest')));
      },
    );

    test('rejects invalid versions before downloading', () async {
      await expectLater(update(requested: '../invalid'), throwsFormatException);
      expect(requests, isEmpty);
    });
  });
}
