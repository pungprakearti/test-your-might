# scale.ps1 -Device '\\.\DISPLAY2' [-Set <percent>]
# Reads (and optionally sets) one monitor's display scaling, like Settings >
# Display > Scale. Only touches the named device.
param([string]$Device, [int]$Set = 0)
Add-Type @"
using System; using System.Runtime.InteropServices;
public class DS {
  [StructLayout(LayoutKind.Sequential)] public struct LUID { public uint Low; public int High; }
  [StructLayout(LayoutKind.Sequential)] public struct PATH_SOURCE { public LUID adapterId; public uint id; public uint modeInfoIdx; public uint statusFlags; }
  [StructLayout(LayoutKind.Sequential)] public struct PATH_TARGET { public LUID adapterId; public uint id; public uint modeInfoIdx; public uint outputTechnology; public uint rotation; public uint scaling; public uint rrNum; public uint rrDen; public uint scanLineOrdering; public int targetAvailable; public uint statusFlags; }
  [StructLayout(LayoutKind.Sequential)] public struct PATH_INFO { public PATH_SOURCE sourceInfo; public PATH_TARGET targetInfo; public uint flags; }
  [StructLayout(LayoutKind.Sequential, Size=64)] public struct MODE_INFO { public uint infoType; }
  [StructLayout(LayoutKind.Sequential)] public struct HEADER { public int type; public uint size; public LUID adapterId; public uint id; }
  [StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode)] public struct SOURCE_NAME { public HEADER header; [MarshalAs(UnmanagedType.ByValTStr, SizeConst=32)] public string name; }
  [StructLayout(LayoutKind.Sequential)] public struct GET_SCALE { public HEADER header; public int minRel; public int curRel; public int maxRel; }
  [StructLayout(LayoutKind.Sequential)] public struct SET_SCALE { public HEADER header; public int rel; }
  [DllImport("user32.dll")] public static extern int GetDisplayConfigBufferSizes(uint f, out uint p, out uint m);
  [DllImport("user32.dll")] public static extern int QueryDisplayConfig(uint f, ref uint np, [Out] PATH_INFO[] p, ref uint nm, [Out] MODE_INFO[] m, IntPtr t);
  [DllImport("user32.dll")] public static extern int DisplayConfigGetDeviceInfo(ref SOURCE_NAME r);
  [DllImport("user32.dll")] public static extern int DisplayConfigGetDeviceInfo(ref GET_SCALE r);
  [DllImport("user32.dll")] public static extern int DisplayConfigSetDeviceInfo(ref SET_SCALE r);
  public static readonly int[] Steps = {100,125,150,175,200,225,250,300,350,400,450,500};
  public static string Run(string dev, int set) {
    uint np, nm; GetDisplayConfigBufferSizes(2, out np, out nm);
    var p = new PATH_INFO[np]; var m = new MODE_INFO[nm];
    int e = QueryDisplayConfig(2, ref np, p, ref nm, m, IntPtr.Zero); if (e != 0) return "query failed " + e;
    for (int i = 0; i < np; i++) {
      var s = p[i].sourceInfo;
      var n = new SOURCE_NAME(); n.header.type = 1; n.header.size = (uint)Marshal.SizeOf(n); n.header.adapterId = s.adapterId; n.header.id = s.id;
      DisplayConfigGetDeviceInfo(ref n);
      if (n.name != dev) continue;
      var g = new GET_SCALE(); g.header.type = -3; g.header.size = (uint)Marshal.SizeOf(g); g.header.adapterId = s.adapterId; g.header.id = s.id;
      DisplayConfigGetDeviceInfo(ref g);
      int rec = -g.minRel; // minRel is relative to recommended, so this is recommended's index
      string info = dev + " current=" + Steps[rec + g.curRel] + "% recommended=" + Steps[rec] + "%";
      if (set == 0) return info;
      int target = Array.IndexOf(Steps, set) - rec;
      var st = new SET_SCALE(); st.header.type = -4; st.header.size = (uint)Marshal.SizeOf(st); st.header.adapterId = s.adapterId; st.header.id = s.id; st.rel = target;
      int r = DisplayConfigSetDeviceInfo(ref st);
      return info + " -> set " + set + "% result=" + r;
    }
    return "device not found";
  }
}
"@
[DS]::Run($Device, $Set)
