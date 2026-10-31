Add-Type -AssemblyName System.Drawing

function Generate-CodeImage($text, $outputPath, $isDark) {
    $bmp = New-Object System.Drawing.Bitmap 1000, 400
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::ClearTypeGridFit

    if ($isDark) {
        $bgColor = [System.Drawing.Color]::FromArgb(30, 30, 30)
        $textColor = [System.Drawing.Color]::FromArgb(230, 230, 230)
    } else {
        $bgColor = [System.Drawing.Color]::FromArgb(255, 255, 255)
        $textColor = [System.Drawing.Color]::FromArgb(30, 30, 30)
    }

    $g.Clear($bgColor)
    $font = New-Object System.Drawing.Font('Consolas', [float]15.0, [System.Drawing.FontStyle]::Regular)
    $brush = New-Object System.Drawing.SolidBrush($textColor)

    $rect = New-Object System.Drawing.RectangleF([float]30.0, [float]30.0, [float]940.0, [float]340.0)
    $g.DrawString($text, $font, $brush, $rect)

    $bmp.Save($outputPath, [System.Drawing.Imaging.ImageFormat]::Png)
    $brush.Dispose()
    $font.Dispose()
    $g.Dispose()
    $bmp.Dispose()
}

$sampleCode = @"
import os
import sys

def calculate_metric(param_a: int, param_b: int = 42) -> dict:
    # Compute syntax and verify operators:
    result = (param_a + param_b) * 2
    tag = "metric_result"
    return {'result': result, 'tag': tag}
"@

$darkPath = Join-Path $PSScriptRoot "code_dark.png"
$lightPath = Join-Path $PSScriptRoot "code_light.png"

Generate-CodeImage $sampleCode $darkPath $true
Generate-CodeImage $sampleCode $lightPath $false
Write-Output "Successfully generated: $darkPath and $lightPath"
