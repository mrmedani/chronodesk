import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:math';
import 'package:http/http.dart' as http;
import 'ffi/native.dart' as native;

const _repo = 'mrmedani/chronodesk';

String get currentVersion => native.getVersion();

class UpdateInfo {
  final String version;
  final String notes;

  UpdateInfo({required this.version, required this.notes});
}

Future<UpdateInfo?> checkForUpdate() async {
  final client = http.Client();
  try {
    final uri = Uri.parse('https://api.github.com/repos/$_repo/releases/latest');
    final resp = await client
        .get(uri, headers: {'Accept': 'application/vnd.github.v3+json'})
        .timeout(const Duration(seconds: 10));
    if (resp.statusCode != 200) return null;

    final data = jsonDecode(resp.body);
    final tag = _asString(data['tag_name']);
    if (tag.isEmpty) return null;

    final clean = tag.startsWith('v') || tag.startsWith('V')
        ? tag.substring(1)
        : tag;
    if (clean == currentVersion) return null;
    if (_compareVersions(clean, currentVersion) <= 0) return null;

    return UpdateInfo(
      version: clean,
      notes: _asString(data['body']),
    );
  } finally {
    client.close();
  }
}

String _asString(dynamic v) {
  if (v is String) return v;
  return '';
}

int _compareVersions(String a, String b) {
  final aParts = a.split('-');
  final bParts = b.split('-');

  final cmp = _compareSemver(aParts[0], bParts[0]);
  if (cmp != 0) return cmp;

  if (aParts.length == 1 && bParts.length == 1) return 0;
  if (aParts.length > 1 && bParts.length == 1) return -1;
  if (aParts.length == 1 && bParts.length > 1) return 1;

  return aParts[1].compareTo(bParts[1]);
}

int _compareSemver(String a, String b) {
  final pa = _parseSegments(a);
  final pb = _parseSegments(b);
  final maxLen = pa.length > pb.length ? pa.length : pb.length;
  for (int i = 0; i < maxLen; i++) {
    final va = i < pa.length ? pa[i] : 0;
    final vb = i < pb.length ? pb[i] : 0;
    if (va != vb) return va.compareTo(vb);
  }
  return 0;
}

List<int> _parseSegments(String v) {
  final parts = v.split('.');
  final result = <int>[];
  for (final p in parts) {
    final n = int.tryParse(p);
    if (n == null) break;
    result.add(n);
  }
  return result;
}

Future<void> downloadAndApplyUpdate(
    void Function(int received, int total) onProgress) async {
  if (!Platform.isWindows) {
    throw UnsupportedError('Updates are only supported on Windows');
  }

  final tmpDir = Directory.systemTemp.path;
  final rnd = Random().nextInt(0x7FFFFFFF);
  final installerFile = File('$tmpDir\\chronodesk_setup_$rnd.exe');

  final uri = Uri.parse(
      'https://github.com/$_repo/releases/latest/download/chronodesk-windows-setup.exe');
  final client = http.Client();
  _downloadClient = client;
  try {
    final request = http.Request('GET', uri);
    final response =
        await client.send(request).timeout(const Duration(minutes: 5));
    if (response.statusCode != 200) {
      throw Exception('Download failed: ${response.statusCode}');
    }

    final total = response.contentLength;
    int received = 0;
    final sink = installerFile.openWrite();

    try {
      await response.stream
          .transform(_progressTransformer((chunkLen) {
            received += chunkLen;
            if (total != null && total > 0) {
              onProgress(received, total);
            } else {
              onProgress(received, 0);
            }
          }))
          .pipe(sink);
    } catch (e) {
      await sink.flush();
      await sink.close();
      if (await installerFile.exists()) {
        await installerFile.delete();
      }
      rethrow;
    }
    await sink.close();
    if (total != null && total > 0) {
      onProgress(total, total);
    }

    await _verifyChecksum(installerFile.path);

    await _runInstaller(installerFile.path);
  } finally {
    _downloadClient = null;
    client.close();
  }
}

Future<void> _verifyChecksum(String installerPath) async {
  final checksumUri = Uri.parse(
      'https://github.com/$_repo/releases/latest/download/chronodesk-windows-setup.exe.sha256');
  final client = http.Client();
  try {
    final resp = await client.get(checksumUri).timeout(const Duration(seconds: 10));
    if (resp.statusCode != 200) return;

    final lines = resp.body.trim().split('\n');
    if (lines.isEmpty) return;
    final expectedHash = lines.first.trim().split(' ').first;
    if (expectedHash.isEmpty || expectedHash.length != 64) return;

    final result = await Process.run('certutil', ['-hashfile', installerPath, 'SHA256']);
    if (result.exitCode != 0) return;
    final output = result.stdout.toString().trim();
    final actualHash = output.split('\n').skip(1).first.trim().replaceAll(' ', '');

    if (actualHash != expectedHash) {
      File(installerPath).delete();
      throw Exception(
          'Installer checksum mismatch. The download may be corrupted or tampered with.');
    }
  } catch (e) {
    if (e is Exception) rethrow;
  } finally {
    client.close();
  }
}

http.Client? _downloadClient;

void cancelUpdate() {
  _downloadClient?.close();
  _downloadClient = null;
}

String _encodePowerShell(String command) {
  final bytes = <int>[];
  for (final unit in command.codeUnits) {
    bytes.add(unit & 0xFF);
    bytes.add((unit >> 8) & 0xFF);
  }
  return base64Encode(bytes);
}

Future<void> _runInstaller(String installerPath) async {
  final exeName = Platform.resolvedExecutable
      .split('\\')
      .last
      .replaceAll('.exe', '');

  final script = '''
\$installer = "$installerPath"
\$exeName = "$exeName"
Write-Host "Waiting for \$exeName to exit..."
while ((Get-Process -Name \$exeName -ErrorAction SilentlyContinue) -ne \$null) {
    Start-Sleep -Milliseconds 300
}
Write-Host "Running installer..."
Start-Process -FilePath \$installer -ArgumentList "/VERYSILENT","/CLOSEAPPLICATIONS","/NORESTART" -Wait
Write-Host "Cleaning up..."
Remove-Item \$installer -Force
''';

  final encoded = _encodePowerShell(script);

  await Process.start('powershell', [
    '-NoProfile', '-ExecutionPolicy', 'Bypass',
    '-Command',
    'Start-Process', 'powershell',
    '-ArgumentList', '\'-NoProfile\', \'-ExecutionPolicy\', \'Bypass\', \'-EncodedCommand\', \'$encoded\'',
    '-Verb', 'RunAs',
    '-WindowStyle', 'Hidden'
  ], mode: ProcessStartMode.detached);

  await Future.delayed(const Duration(seconds: 3));
  exit(0);
}

StreamTransformer<List<int>, List<int>> _progressTransformer(
    void Function(int chunkLen) onData) {
  return StreamTransformer.fromHandlers(
    handleData: (chunk, sink) {
      onData(chunk.length);
      sink.add(chunk);
    },
  );
}
