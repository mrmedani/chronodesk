import 'dart:async';
import 'dart:convert';
import 'dart:io';
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
  if (aParts.length == 1) return 1;
  if (bParts.length == 1) return -1;

  return _comparePreRelease(aParts[1], bParts[1]);
}

int _comparePreRelease(String a, String b) {
  final aParts = a.split('.');
  final bParts = b.split('.');
  final maxLen = aParts.length > bParts.length ? aParts.length : bParts.length;
  for (int i = 0; i < maxLen; i++) {
    if (i >= aParts.length) return -1;
    if (i >= bParts.length) return 1;
    final aNum = int.tryParse(aParts[i]);
    final bNum = int.tryParse(bParts[i]);
    if (aNum != null && bNum != null) {
      if (aNum != bNum) return aNum.compareTo(bNum);
    } else {
      final cmp = aParts[i].compareTo(bParts[i]);
      if (cmp != 0) return cmp;
    }
  }
  return 0;
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

  _cleanupStaleScripts();

  final tmpDir = Directory.systemTemp.path;
  final installerFile = File('$tmpDir\\chronodesk_setup_$pid.exe');

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
      await _safeDelete(installerFile.path);
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
    if (resp.statusCode != 200) {
      stderr.writeln(
          '[Update] Checksum unavailable (HTTP ${resp.statusCode}), skipping');
      return;
    }

    final body = resp.body.trim();
    if (body.isEmpty) {
      stderr.writeln('[Update] Checksum file empty, skipping');
      return;
    }

    final expectedMatch = RegExp(r'[a-fA-F0-9]{64}').firstMatch(body);
    if (expectedMatch == null) {
      await _safeDelete(installerPath);
      throw Exception('No valid SHA256 hash found in checksum file.');
    }
    final expectedHash = expectedMatch.group(0)!.toLowerCase();

    final result =
        await Process.run('certutil', ['-hashfile', installerPath, 'SHA256']);
    if (result.exitCode != 0) {
      stderr.writeln('[Update] certutil failed, skipping verification');
      return;
    }

    final actualMatch =
        RegExp(r'[a-fA-F0-9]{64}').firstMatch(result.stdout.toString());
    if (actualMatch == null) {
      stderr.writeln(
          '[Update] certutil output has no hash, skipping verification');
      return;
    }

    final actualHash = actualMatch.group(0)!.toLowerCase();
    if (actualHash != expectedHash) {
      await _safeDelete(installerPath);
      throw Exception(
          'Installer checksum mismatch. The download may be corrupted or tampered with.');
    }
  } finally {
    client.close();
  }
}

Future<void> _safeDelete(String path) async {
  try {
    final f = File(path);
    if (await f.exists()) await f.delete();
  } catch (_) {}
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

void _cleanupStaleScripts() {
  try {
    final tmpDir = Directory.systemTemp.path;
    for (final f in Directory(tmpDir).listSync()) {
      if (f is File &&
          f.path.endsWith('.ps1') &&
          f.path.contains('chronodesk_update_')) {
        try {
          f.delete();
        } catch (_) {}
      }
    }
  } catch (_) {}
}

Future<void> _runInstaller(String installerPath) async {
  final installDir = File(Platform.resolvedExecutable).parent.path;
  final currentPid = pid;
  final scriptPath =
      '${Directory.systemTemp.path}\\chronodesk_update_$currentPid.ps1';

  final safeInstaller = installerPath.replaceAll('"', '`"');
  final safeInstallDir = installDir.replaceAll('"', '`"');
  final safeScriptPath = scriptPath.replaceAll("'", "''");

  final scriptContent = '''
\$installerPath = "$safeInstaller"
\$installDir = "$safeInstallDir"
\$targetPid = $currentPid

Write-Host "[Chronodesk Update] Waiting for PID \$targetPid to exit..."
while (Get-Process -Id \$targetPid -ErrorAction SilentlyContinue) {
    Start-Sleep -Milliseconds 300
}

Write-Host "[Chronodesk Update] Installing update..."
\$dirArg = [string]::Format('/DIR="{0}"', \$installDir)
\$p = Start-Process -FilePath \$installerPath -ArgumentList '/VERYSILENT','/CLOSEAPPLICATIONS','/NORESTART',\$dirArg -Wait -PassThru -NoNewWindow

if (\$p.ExitCode -eq 0) {
    Write-Host "[Chronodesk Update] Installation successful, restarting..."
    Start-Process -FilePath "\$installDir\\chronodesk.exe" -WindowStyle Normal
} else {
    Write-Host "[Chronodesk Update] Installer returned exit code \$(\$p.ExitCode)"
    Add-Type -AssemblyName System.Windows.Forms
    \$notify = New-Object System.Windows.Forms.NotifyIcon
    \$notify.Text = 'Chronodesk'
    \$notify.Icon = [System.Drawing.SystemIcons]::Error
    \$notify.BalloonTipTitle = 'Update Failed'
    \$notify.BalloonTipText = "Installer error \$(\$p.ExitCode). Please run the update again."
    \$notify.Visible = \$true
    \$notify.ShowBalloonTip(5000)
}

Remove-Item -LiteralPath \$installerPath -Force -ErrorAction SilentlyContinue
Write-Host "[Chronodesk Update] Update script complete"
''';

  final scriptFile = File(scriptPath);
  await scriptFile.writeAsString(scriptContent);

  final outerCmd =
      "Start-Process powershell -Verb RunAs -WindowStyle Hidden -ArgumentList '-NoProfile','-ExecutionPolicy','Bypass','-File','$safeScriptPath'";
  final encoded = _encodePowerShell(outerCmd);

  await Process.start('powershell', [
    '-NoProfile',
    '-ExecutionPolicy',
    'Bypass',
    '-EncodedCommand',
    encoded,
  ], mode: ProcessStartMode.detached);

  try { scriptFile.deleteSync(); } catch (_) {}

  await Future.delayed(const Duration(seconds: 1));
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
