import 'dart:ffi';
import 'dart:io';
import 'package:ffi/ffi.dart';

final DynamicLibrary _lib = _loadLibrary();

DynamicLibrary _loadLibrary() {
  if (Platform.isWindows) {
    return DynamicLibrary.open('chronodesk.dll');
  } else if (Platform.isMacOS) {
    return DynamicLibrary.open('libchronodesk.dylib');
  } else {
    return DynamicLibrary.open('libchronodesk.so');
  }
}

final chronodeskInit = _lib.lookupFunction<
    Void Function(Pointer<Utf8>),
    void Function(Pointer<Utf8>)>('chronodesk_init');

final chronodeskGetPeerId = _lib.lookupFunction<
    Pointer<Utf8> Function(),
    Pointer<Utf8> Function()>('chronodesk_get_peer_id');

final chronodeskFreeString = _lib.lookupFunction<
    Void Function(Pointer<Utf8>),
    void Function(Pointer<Utf8>)>('chronodesk_free_string');

final chronodeskPollEvent = _lib.lookupFunction<
    Pointer<Utf8> Function(),
    Pointer<Utf8> Function()>('chronodesk_poll_event');

final chronodeskConnect = _lib.lookupFunction<
    Void Function(Pointer<Utf8>),
    void Function(Pointer<Utf8>)>('chronodesk_connect');

final chronodeskAccept = _lib.lookupFunction<
    Void Function(),
    void Function()>('chronodesk_accept');

final chronodeskDeny = _lib.lookupFunction<
    Void Function(),
    void Function()>('chronodesk_deny');

final chronodeskDisconnect = _lib.lookupFunction<
    Void Function(),
    void Function()>('chronodesk_disconnect');

typedef GetFrameNative = Int32 Function(
  Pointer<Pointer<Uint8>>,
  Pointer<Int32>,
  Pointer<Int32>,
  Pointer<Int32>,
);
typedef GetFrameDart = int Function(
  Pointer<Pointer<Uint8>>,
  Pointer<Int32>,
  Pointer<Int32>,
  Pointer<Int32>,
);
final chronodeskGetFrame = _lib.lookupFunction<GetFrameNative, GetFrameDart>('chronodesk_get_frame');

final chronodeskFreeFrame = _lib.lookupFunction<
    Void Function(Pointer<Uint8>),
    void Function(Pointer<Uint8>)>('chronodesk_free_frame');

final chronodeskSendInputMove = _lib.lookupFunction<
    Void Function(Int32, Int32),
    void Function(int, int)>('chronodesk_send_input_move');

final chronodeskSendInputClick = _lib.lookupFunction<
    Void Function(Uint8, Bool),
    void Function(int, bool)>('chronodesk_send_input_click');

String? _readCString(Pointer<Utf8> ptr) {
  if (ptr == nullptr) return null;
  final s = ptr.toDartString();
  chronodeskFreeString(ptr);
  return s;
}

String getPeerId() {
  final ptr = chronodeskGetPeerId();
  return _readCString(ptr) ?? '';
}

String? pollEvent() {
  final ptr = chronodeskPollEvent();
  final s = _readCString(ptr);
  if (s != null && s.isEmpty) return null;
  return s;
}

bool getFrame(Pointer<Pointer<Uint8>> data, Pointer<Int32> len,
    Pointer<Int32> w, Pointer<Int32> h) {
  return chronodeskGetFrame(data, len, w, h) != 0;
}
