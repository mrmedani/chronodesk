import 'dart:ffi';
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

typedef StartHostNative = Void Function(Pointer<Utf8> signalingAddr, Pointer<Utf8> peerId);
typedef StartHostDart = void Function(Pointer<Utf8> signalingAddr, Pointer<Utf8> peerId);

final StartHostDart startHost = _lib
    .lookupFunction<StartHostNative, StartHostDart>('start_host');

typedef StartClientNative = Void Function(
    Pointer<Utf8> signalingAddr, Pointer<Utf8> peerId, Pointer<Utf8> connectTo);
typedef StartClientDart = void Function(
    Pointer<Utf8> signalingAddr, Pointer<Utf8> peerId, Pointer<Utf8> connectTo);

final StartClientDart startClient = _lib
    .lookupFunction<StartClientNative, StartClientDart>('start_client');
