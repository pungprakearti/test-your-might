# Minimal XSETTINGS manager: owns _XSETTINGS_S0 and publishes Xft/DPI.
# usage: python3 tools/xsettings.py <dpi> [<dpi2> <after secs>]  (runs until killed)
import ctypes, struct, sys, time
X = ctypes.cdll.LoadLibrary("libX11.so.6")
X.XOpenDisplay.restype = ctypes.c_void_p
X.XDefaultRootWindow.restype = ctypes.c_ulong
X.XCreateSimpleWindow.restype = ctypes.c_ulong
X.XInternAtom.restype = ctypes.c_ulong
for f in ("XDefaultRootWindow", "XCreateSimpleWindow", "XInternAtom", "XSetSelectionOwner", "XChangeProperty", "XFlush", "XSync"):
    getattr(X, f).argtypes = None
d = ctypes.c_void_p(X.XOpenDisplay(None))
root = ctypes.c_ulong(X.XDefaultRootWindow(d))
w = ctypes.c_ulong(X.XCreateSimpleWindow(d, root, 0, 0, 1, 1, 0, 0, 0))
sel = ctypes.c_ulong(X.XInternAtom(d, b"_XSETTINGS_S0", 0))
prop = ctypes.c_ulong(X.XInternAtom(d, b"_XSETTINGS_SETTINGS", 0))
serial = 0
def publish(dpi):
    global serial
    name = b"Xft/DPI"
    pad = (4 - len(name) % 4) % 4
    body = struct.pack("<BxH", 0, len(name)) + name + b"\0" * pad + struct.pack("<Ii", serial, int(dpi * 1024))
    data = struct.pack("<B3xII", ord("l"), serial, 1) + body
    serial += 1
    X.XChangeProperty(d, w, prop, prop, 8, 0, data, len(data))
    X.XSync(d, 0)
    print(f"xsettings: Xft/DPI = {dpi}", flush=True)
publish(float(sys.argv[1]))
X.XSetSelectionOwner(d, sel, w, 0)
X.XSync(d, 0)
if len(sys.argv) > 3:
    time.sleep(float(sys.argv[3]))
    publish(float(sys.argv[2]))
while True:
    time.sleep(1)
