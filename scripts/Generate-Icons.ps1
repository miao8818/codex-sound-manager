$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing

$root = Split-Path -Parent $PSScriptRoot
$icons = Join-Path $root 'icons'
New-Item -ItemType Directory -Path $icons -Force | Out-Null

$size = 512
$bitmap = New-Object System.Drawing.Bitmap($size, $size)
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
$graphics.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
$graphics.Clear([System.Drawing.Color]::Transparent)

$path = New-Object System.Drawing.Drawing2D.GraphicsPath
$radius = 92
$rect = New-Object System.Drawing.Rectangle(20, 20, 472, 472)
$diameter = $radius * 2
$path.AddArc($rect.Left, $rect.Top, $diameter, $diameter, 180, 90)
$path.AddArc($rect.Right - $diameter, $rect.Top, $diameter, $diameter, 270, 90)
$path.AddArc($rect.Right - $diameter, $rect.Bottom - $diameter, $diameter, $diameter, 0, 90)
$path.AddArc($rect.Left, $rect.Bottom - $diameter, $diameter, $diameter, 90, 90)
$path.CloseFigure()

$background = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(255, 24, 31, 42))
$graphics.FillPath($background, $path)

$white = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::White)
$teal = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(255, 20, 184, 166))
$bars = @(
    @{ X = 132; Y = 210; W = 46; H = 92; Brush = $white },
    @{ X = 200; Y = 152; W = 46; H = 208; Brush = $teal },
    @{ X = 268; Y = 184; W = 46; H = 144; Brush = $white },
    @{ X = 336; Y = 226; W = 46; H = 60; Brush = $teal }
)
foreach ($bar in $bars) {
    $barPath = New-Object System.Drawing.Drawing2D.GraphicsPath
    $barRadius = 22
    $barRect = New-Object System.Drawing.Rectangle($bar.X, $bar.Y, $bar.W, $bar.H)
    $barDiameter = $barRadius * 2
    $barPath.AddArc($barRect.Left, $barRect.Top, $barDiameter, $barDiameter, 180, 90)
    $barPath.AddArc($barRect.Right - $barDiameter, $barRect.Top, $barDiameter, $barDiameter, 270, 90)
    $barPath.AddArc($barRect.Right - $barDiameter, $barRect.Bottom - $barDiameter, $barDiameter, $barDiameter, 0, 90)
    $barPath.AddArc($barRect.Left, $barRect.Bottom - $barDiameter, $barDiameter, $barDiameter, 90, 90)
    $barPath.CloseFigure()
    $graphics.FillPath($bar.Brush, $barPath)
    $barPath.Dispose()
}

$output = Join-Path $icons 'app-icon.png'
$bitmap.Save($output, [System.Drawing.Imaging.ImageFormat]::Png)

$teal.Dispose()
$white.Dispose()
$background.Dispose()
$path.Dispose()
$graphics.Dispose()
$bitmap.Dispose()

Write-Output $output
