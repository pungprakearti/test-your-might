# drive.ps1 -Proc <process name> -Action <shot|dblclick|dragto|maximize|close> [-Out <png>]
#           [-X <x> -Y <y>] [-TX <x> -TY <y>]
# Drives a running Test Your Might window on the Windows desktop with the real
# mouse and reports its rect/DPI; -Out saves a screenshot of the window.
# -X/-Y: grab point inside the window (physical px); -TX/-TY: drag target on
# the desktop.
param([string]$Proc = "test-your-might", [string]$Action, [string]$Out, [int]$X = 250, [int]$Y = 60, [int]$TX = 0, [int]$TY = 0)
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System; using System.Runtime.InteropServices;
public class W {
  [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr v);
  [DllImport("user32.dll")] public static extern IntPtr FindWindow(string c, string n);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int l,t,r,b; }
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool IsZoomed(IntPtr h);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(int f, int x, int y, int d, int e);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, int m, IntPtr w, IntPtr l);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
}
"@
[W]::SetProcessDpiAwarenessContext([IntPtr](-4)) | Out-Null
$h = [IntPtr](Get-Process -Name $Proc -ErrorAction SilentlyContinue | Select-Object -First 1).MainWindowHandle
if ($h -eq [IntPtr]::Zero) { "no window"; exit 1 }
$r = New-Object W+RECT; [W]::GetWindowRect($h, [ref]$r) | Out-Null
function Report { [W]::GetWindowRect($h, [ref]$script:r) | Out-Null
  "rect=$($r.l),$($r.t) size=$($r.r-$r.l)x$($r.b-$r.t) maximized=$([W]::IsZoomed($h)) dpi=$([W]::GetDpiForWindow($h))" }
if ($Action -eq "dblclick") {
  [W]::SetForegroundWindow($h) | Out-Null
  [W]::SetCursorPos($r.l + $X, $r.t + $Y) | Out-Null; Start-Sleep -Milliseconds 200
  1..2 | ForEach-Object { [W]::mouse_event(2,0,0,0,0); Start-Sleep -Milliseconds 30; [W]::mouse_event(4,0,0,0,0); Start-Sleep -Milliseconds 80 }
  Start-Sleep -Milliseconds 1500
}
if ($Action -eq "dragto") {
  # Drag by the cabinet art to screen point ($TX,$TY) in small steps.
  [W]::SetForegroundWindow($h) | Out-Null
  $sx = $r.l + $X; $sy = $r.t + $Y
  [W]::SetCursorPos($sx, $sy) | Out-Null; Start-Sleep -Milliseconds 200
  [W]::mouse_event(2,0,0,0,0); Start-Sleep -Milliseconds 150
  for ($i = 1; $i -le 30; $i++) { [W]::SetCursorPos([int]($sx + ($TX-$sx)*$i/30), [int]($sy + ($TY-$sy)*$i/30)) | Out-Null; Start-Sleep -Milliseconds 25 }
  Start-Sleep -Milliseconds 700
  [W]::mouse_event(4,0,0,0,0); Start-Sleep -Milliseconds 1500
}
if ($Action -eq "maximize") { [W]::PostMessage($h, 0x112, [IntPtr]0xF030, [IntPtr]::Zero) | Out-Null; Start-Sleep -Milliseconds 150; "after 150ms: maximized=$([W]::IsZoomed($h))"; Start-Sleep -Milliseconds 1500 }
if ($Action -eq "close") { [W]::PostMessage($h, 0x10, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null; Start-Sleep 1; "closed"; exit }
Report
if ($Out) {
  $w = $r.r - $r.l; $hh = $r.b - $r.t
  $bmp = New-Object System.Drawing.Bitmap $w, $hh
  $g = [System.Drawing.Graphics]::FromImage($bmp); $g.CopyFromScreen($r.l, $r.t, 0, 0, $bmp.Size)
  $bmp.Save($Out); "saved $Out"
}
