# Gera assets\icon.ico (PNG dentro de ICO) do Screenshotter. Rodar: powershell -File tools\make_icon.ps1
Add-Type -AssemblyName System.Drawing
$root = Split-Path -Parent $PSScriptRoot
$sizes = 16, 24, 32, 48, 64, 128, 256

function New-IconBitmap([int]$s) {
  $bmp = New-Object System.Drawing.Bitmap $s, $s, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.SmoothingMode = 'AntiAlias'; $g.PixelOffsetMode = 'HighQuality'
  $f = $s / 256.0
  # Fundo arredondado com degradê indigo -> ciano
  $r = [int](56 * $f); $w = $s - 1
  $path = New-Object System.Drawing.Drawing2D.GraphicsPath
  $d = 2 * $r
  $path.AddArc(0, 0, $d, $d, 180, 90); $path.AddArc($w - $d, 0, $d, $d, 270, 90)
  $path.AddArc($w - $d, $w - $d, $d, $d, 0, 90); $path.AddArc(0, $w - $d, $d, $d, 90, 90)
  $path.CloseFigure()
  $grad = New-Object System.Drawing.Drawing2D.LinearGradientBrush(
    (New-Object System.Drawing.PointF 0, 0), (New-Object System.Drawing.PointF $s, $s),
    [System.Drawing.Color]::FromArgb(255, 79, 70, 229), [System.Drawing.Color]::FromArgb(255, 6, 182, 212))
  $g.FillPath($grad, $path)
  # Cantos de enquadramento (captura) em branco
  $pen = New-Object System.Drawing.Pen ([System.Drawing.Color]::White), ([single](22 * $f))
  $pen.StartCap = 'Round'; $pen.EndCap = 'Round'
  $m = 56 * $f; $L = 44 * $f
  $a = [single]$m; $b = [single]($s - $m); $l = [single]$L
  $g.DrawLine($pen, $a, $a + $l, $a, $a); $g.DrawLine($pen, $a, $a, $a + $l, $a)
  $g.DrawLine($pen, $b - $l, $a, $b, $a); $g.DrawLine($pen, $b, $a, $b, $a + $l)
  $g.DrawLine($pen, $a, $b - $l, $a, $b); $g.DrawLine($pen, $a, $b, $a + $l, $b)
  $g.DrawLine($pen, $b - $l, $b, $b, $b); $g.DrawLine($pen, $b, $b - $l, $b, $b)
  # "Pegador": ponto vermelho no centro
  $dot = [single](56 * $f)
  $brush = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(255, 248, 113, 113))
  $c = $s / 2.0 - $dot / 2.0
  $g.FillEllipse($brush, [single]$c, [single]$c, $dot, $dot)
  $g.Dispose(); return $bmp
}

$pngs = @()
foreach ($s in $sizes) {
  $bmp = New-IconBitmap $s
  $ms = New-Object System.IO.MemoryStream
  $bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
  $pngs += ,@{ size = $s; bytes = $ms.ToArray() }
  $bmp.Dispose()
}

$out = Join-Path $root "assets\icon.ico"
$fs = [System.IO.File]::Open($out, 'Create')
$bw = New-Object System.IO.BinaryWriter $fs
$bw.Write([uint16]0); $bw.Write([uint16]1); $bw.Write([uint16]$pngs.Count)
$offset = 6 + 16 * $pngs.Count
foreach ($p in $pngs) {
  $dim = if ($p.size -ge 256) { 0 } else { $p.size }
  $bw.Write([byte]$dim); $bw.Write([byte]$dim); $bw.Write([byte]0); $bw.Write([byte]0)
  $bw.Write([uint16]1); $bw.Write([uint16]32)
  $bw.Write([uint32]$p.bytes.Length); $bw.Write([uint32]$offset)
  $offset += $p.bytes.Length
}
foreach ($p in $pngs) { $bw.Write($p.bytes) }
$bw.Close(); $fs.Close()
"wrote $out ($((Get-Item $out).Length) bytes)"

