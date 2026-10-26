import 'dart:convert';
import 'dart:ffi';
import 'dart:io';

import 'package:crypto/crypto.dart';
import 'package:falcon/src/release.dart';
import 'package:falcon/src/version.dart';

Future<void> main(List<String> args) async {
  try {
    final platform = releasePlatform(Abi.current());
    final home = Platform.environment['HOME'];
    final xdgCache = Platform.environment['XDG_CACHE_HOME'];
    final String cache;
    if (Platform.isLinux && xdgCache != null && xdgCache.startsWith('/')) {
      cache = xdgCache;
    } else {
      if (home == null || home.isEmpty) {
        throw StateError('HOME is not set; cannot locate the falcon cache.');
      }
      cache = Platform.isMacOS ? '$home/Library/Caches' : '$home/.cache';
    }
    final cacheDirectory = Directory('$cache/falcon');
    final pinnedVersion = Platform.environment['FALCON_VERSION'];
    if (args.isNotEmpty && args.first == 'self-update') {
      if (args.length > 2) {
        throw ArgumentError('Usage: falcon self-update [VERSION]');
      }
      final updated = await selfUpdate(
        cacheDirectory,
        platform,
        requestedVersion: args.length == 2 ? args[1] : null,
        pinnedVersion: pinnedVersion,
      );
      stdout.writeln('falcon: installed $updated.');
      return;
    }
    final selected = await selectedVersion(
      cacheDirectory,
      pinnedVersion: pinnedVersion,
    );
    final directory = Directory('${cacheDirectory.path}/$selected');
    final binary = File('${directory.path}/falcon');
    if (!await binary.exists()) {
      await install(directory, binary, platform, selected);
    }
    final process = await Process.start(
      binary.path,
      args,
      mode: ProcessStartMode.inheritStdio,
    );
    exitCode = await process.exitCode;
  } catch (error) {
    stderr.writeln('falcon: $error');
    exitCode = 1;
  }
}

Future<void> install(
  Directory directory,
  File binary,
  String platform,
  String selected, {
  String releasesUrl = 'https://github.com/JacobDevelops/dart_falcon/releases',
}) async {
  await directory.create(recursive: true);
  final temporary = await directory.createTemp('.download-');
  final client = HttpClient();
  try {
    final asset = 'falcon-$selected-$platform.tar.gz';
    final base = '$releasesUrl/download/v$selected';
    final archive = File('${temporary.path}/$asset');
    final response = await download(client, '$base/$asset');
    await response.pipe(archive.openWrite());
    final sums = await (await download(
      client,
      '$base/SHA256SUMS',
    )).transform(utf8.decoder).join();
    final expected = checksumFor(sums, asset);
    final actual = (await sha256.bind(archive.openRead()).first).toString();
    if (actual != expected) {
      throw StateError('SHA256 checksum mismatch for $asset.');
    }
    final extracted = File('${temporary.path}/falcon');
    final extraction = await Process.run('tar', [
      '-xzf',
      archive.path,
      '-C',
      temporary.path,
      'falcon',
    ]);
    if (extraction.exitCode != 0) {
      throw StateError('Cannot extract $asset: ${extraction.stderr}');
    }
    final chmod = await Process.run('chmod', ['755', extracted.path]);
    if (chmod.exitCode != 0) {
      throw StateError('Cannot make falcon executable: ${chmod.stderr}');
    }
    // Renaming on the same filesystem exposes only a complete verified binary.
    await extracted.rename(binary.path);
  } finally {
    client.close(force: true);
    await temporary.delete(recursive: true);
  }
}

Future<HttpClientResponse> download(HttpClient client, String url) async {
  final response = await (await client.getUrl(Uri.parse(url))).close();
  if (response.statusCode != HttpStatus.ok) {
    throw HttpException(
      'Download failed: HTTP ${response.statusCode}',
      uri: Uri.parse(url),
    );
  }
  return response;
}

Future<String> selectedVersion(Directory cache, {String? pinnedVersion}) async {
  if (pinnedVersion != null) return releaseVersion(pinnedVersion);
  final selection = File('${cache.path}/active-version');
  if (await selection.exists()) {
    final saved = await selection.readAsLines();
    if (saved.length == 2 && saved.first == version) {
      return releaseVersion(saved[1].trim());
    }
  }
  return version;
}

Future<String> selfUpdate(
  Directory cache,
  String platform, {
  String? requestedVersion,
  String? pinnedVersion,
  String releasesUrl = 'https://github.com/JacobDevelops/dart_falcon/releases',
  String latestReleaseUrl =
      'https://api.github.com/repos/JacobDevelops/dart_falcon/releases/latest',
}) async {
  final pin = requestedVersion ?? pinnedVersion;
  final String selected;
  if (pin != null) {
    selected = releaseVersion(pin);
  } else {
    final client = HttpClient();
    try {
      final response = await download(client, latestReleaseUrl);
      final release = jsonDecode(await response.transform(utf8.decoder).join());
      if (release is! Map<String, dynamic> || release['tag_name'] is! String) {
        throw const FormatException('Latest release has no version tag.');
      }
      selected = releaseVersion(release['tag_name'] as String);
    } finally {
      client.close(force: true);
    }
  }
  final directory = Directory('${cache.path}/$selected');
  await install(
    directory,
    File('${directory.path}/falcon'),
    platform,
    selected,
    releasesUrl: releasesUrl,
  );
  final temporary = await cache.createTemp('.selection-');
  try {
    final selection = File('${temporary.path}/active-version');
    await selection.writeAsString('$version\n$selected\n', flush: true);
    await selection.rename('${cache.path}/active-version');
  } finally {
    await temporary.delete(recursive: true);
  }
  return selected;
}
