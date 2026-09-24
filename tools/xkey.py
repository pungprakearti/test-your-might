# tools/xkey.py <keysym name>...  - press keys via XTest into the focused window
import ctypes, sys, time
X = ctypes.cdll.LoadLibrary("libX11.so.6"); T = ctypes.cdll.LoadLibrary("libXtst.so.6")
X.XOpenDisplay.restype = ctypes.c_void_p; X.XStringToKeysym.restype = ctypes.c_ulong
d = ctypes.c_void_p(X.XOpenDisplay(None))
for name in sys.argv[1:]:
    kc = X.XKeysymToKeycode(d, ctypes.c_ulong(X.XStringToKeysym(name.encode())))
    T.XTestFakeKeyEvent(d, kc, 1, 0); X.XFlush(d); time.sleep(0.05)
    T.XTestFakeKeyEvent(d, kc, 0, 0); X.XFlush(d); time.sleep(0.2)
