param([Parameter(Mandatory = $true)][uint32]$DeckProcessId, [ValidateSet('read', 'drag', 'resize')][string]$Action = 'read')
$ErrorActionPreference = 'Stop'
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;
public static class DeckWindowProbe {
  public delegate bool EnumCallback(IntPtr hwnd, IntPtr param);
  [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left,Top,Right,Bottom; }
  [StructLayout(LayoutKind.Sequential)] public struct Point { public int X,Y; }
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumCallback callback, IntPtr param);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hwnd, out uint pid);
  [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr hwnd, StringBuilder text, int size);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hwnd, out Rect rect);
  [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr hwnd, out Rect rect);
  [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr hwnd, ref Point point);
  [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr hwnd);
  [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr context);
  [DllImport("user32.dll")] public static extern IntPtr GetWindowLongPtrW(IntPtr hwnd, int index);
  [DllImport("user32.dll")] public static extern bool GetLayeredWindowAttributes(IntPtr hwnd, out uint color, out byte alpha, out uint flags);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hwnd);
  [DllImport("user32.dll")] public static extern int GetWindowRgn(IntPtr hwnd, IntPtr region);
  [DllImport("gdi32.dll")] public static extern IntPtr CreateRectRgn(int left,int top,int right,int bottom);
  [DllImport("gdi32.dll")] public static extern bool DeleteObject(IntPtr handle);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hwnd);
  [DllImport("user32.dll")] public static extern bool GetCursorPos(out Point point);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x,int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint flags,uint dx,uint dy,uint data,UIntPtr extra);
  public static IntPtr Find(uint process) {
    SetThreadDpiAwarenessContext(new IntPtr(-4));
    IntPtr found=IntPtr.Zero;
    EnumWindows((window,param)=> { uint pid; GetWindowThreadProcessId(window,out pid); if(pid!=process)return true;
      var title=new StringBuilder(256);GetWindowTextW(window,title,title.Capacity);
      if(title.ToString()=="Dushan Deck \u00b7 Quota"){found=window;return false;}return true; },IntPtr.Zero);
    if(found==IntPtr.Zero)throw new Exception("Owned floating window was not found.");return found;
  }
  public static void Gesture(IntPtr window,bool resize) {
    if(!IsWindowVisible(window) || !SetForegroundWindow(window))throw new Exception("Owned window cannot receive input.");
    Rect rect;GetWindowRect(window,out rect);var scale=GetDpiForWindow(window)/96.0;
    int x=resize?rect.Right-(int)(8*scale):rect.Left+(int)(45*scale);
    int y=resize?rect.Bottom-(int)(8*scale):rect.Top+(int)(17*scale);
    Point saved;GetCursorPos(out saved);
    try { SetCursorPos(x,y);Thread.Sleep(40);mouse_event(2,0,0,0,UIntPtr.Zero);Thread.Sleep(80);
      for(int i=1;i<=8;i++){SetCursorPos(x+i*9,y+i*6);Thread.Sleep(25);}
    } finally { mouse_event(4,0,0,0,UIntPtr.Zero);Thread.Sleep(100);SetCursorPos(saved.X,saved.Y); }
  }
}
'@
$window = [DeckWindowProbe]::Find($DeckProcessId)
if ($Action -ne 'read') { [DeckWindowProbe]::Gesture($window, $Action -eq 'resize') }
$rectangle = New-Object DeckWindowProbe+Rect
$client = New-Object DeckWindowProbe+Rect
[DeckWindowProbe]::GetWindowRect($window, [ref]$rectangle) | Out-Null
[DeckWindowProbe]::GetClientRect($window, [ref]$client) | Out-Null
[uint32]$color = 0
[byte]$alpha = 0
[uint32]$flags = 0
if (-not [DeckWindowProbe]::GetLayeredWindowAttributes($window,[ref]$color,[ref]$alpha,[ref]$flags)) { throw 'Unable to read native opacity.' }
$style = [DeckWindowProbe]::GetWindowLongPtrW($window,-16).ToInt64()
$extended = [DeckWindowProbe]::GetWindowLongPtrW($window,-20).ToInt64()
$origin = New-Object DeckWindowProbe+Point
[DeckWindowProbe]::ClientToScreen($window,[ref]$origin) | Out-Null
$region = [DeckWindowProbe]::CreateRectRgn(0,0,0,0)
try { $regionType = [DeckWindowProbe]::GetWindowRgn($window,$region) } finally { [DeckWindowProbe]::DeleteObject($region) | Out-Null }
[ordered]@{ x=$rectangle.Left; y=$rectangle.Top; width=$client.Right; height=$client.Bottom; dpi=[DeckWindowProbe]::GetDpiForWindow($window); alpha=$alpha; rounded=$regionType -eq 3; topmost=($extended -band 8) -ne 0; decorated=($origin.Y - $rectangle.Top) -gt 0; toolWindow=($extended -band 0x80) -ne 0; outerWidth=$rectangle.Right-$rectangle.Left; outerHeight=$rectangle.Bottom-$rectangle.Top; style=$style; extendedStyle=$extended; clientOffsetX=$origin.X-$rectangle.Left; clientOffsetY=$origin.Y-$rectangle.Top } | ConvertTo-Json -Compress
