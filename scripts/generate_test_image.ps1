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

function Generate-LoopCodeImage($text, $outputPath, $isDark) {
    $bmp = New-Object System.Drawing.Bitmap 1000, 420
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

    $y = [float]30.0
    foreach ($line in ($text -split "`r?`n")) {
        $g.DrawString($line, $font, $brush, [float]30.0, $y)
        $y += [float]34.0
    }

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

$benchmarkCode = @"
for i in range(10):
    if (i % 2 == 0):
        print(f"%i is even")
    else:
        print(f"%i is odd")
"@

$loopDarkPath = Join-Path $PSScriptRoot "code_loop_dark.png"
$loopLightPath = Join-Path $PSScriptRoot "code_loop_light.png"

Generate-LoopCodeImage $benchmarkCode $loopDarkPath $true
Generate-LoopCodeImage $benchmarkCode $loopLightPath $false

function Generate-SyntaxBenchmarkImage($outputPath) {
    $bmp = New-Object System.Drawing.Bitmap 480, 320
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::ClearTypeGridFit

    $bg = [System.Drawing.Color]::FromArgb(18, 22, 25)
    $colLineNum = [System.Drawing.Color]::FromArgb(80, 95, 110)
    $colKw = [System.Drawing.Color]::FromArgb(204, 120, 50)
    $colIdent = [System.Drawing.Color]::FromArgb(169, 183, 198)
    $colFunc = [System.Drawing.Color]::FromArgb(104, 151, 187)
    $colNum = [System.Drawing.Color]::FromArgb(152, 118, 170)
    $colStr = [System.Drawing.Color]::FromArgb(106, 135, 89)
    $colOut = [System.Drawing.Color]::FromArgb(128, 128, 128)

    $g.Clear($bg)
    $font = New-Object System.Drawing.Font('Consolas', [float]14.0, [System.Drawing.FontStyle]::Regular)
    $charW = [float]11.0
    $q = [char]34

    $lines = @(
        @(@(1, "1", $colLineNum), @(4, "for", $colKw), @(8, "i", $colIdent), @(10, "in", $colKw), @(13, "range(10):", $colFunc)),
        @(@(1, "2", $colLineNum), @(8, "if", $colKw), @(11, "i", $colIdent), @(13, "%", $colIdent), @(15, "2", $colNum), @(17, "==", $colIdent), @(20, "0:", $colNum)),
        @(@(1, "3", $colLineNum), @(12, "print", $colKw), @(17, "(f", $colIdent), @(19, "${q}%i is even${q}", $colStr), @(31, ")", $colIdent)),
        @(@(1, "4", $colLineNum), @(8, "else:", $colKw)),
        @(@(1, "5", $colLineNum), @(12, "print", $colKw), @(17, "(f", $colIdent), @(19, "${q}%i is odd${q}", $colStr), @(30, ")", $colIdent)),
        @(),
        @(@(1, "7", $colLineNum), @(4, "0 is even", $colOut)),
        @(@(1, "8", $colLineNum), @(4, "1 is odd", $colOut)),
        @(@(1, "9", $colLineNum), @(4, "2 is even", $colOut)),
        @(@(0, "10", $colLineNum), @(4, "3 is odd", $colOut))
    )

    $y = [float]18.0
    foreach ($line in $lines) {
        foreach ($seg in $line) {
            $colIdx = $seg[0]
            $txt = $seg[1]
            $col = $seg[2]
            $brush = New-Object System.Drawing.SolidBrush($col)
            for ($ci = 0; $ci -lt $txt.Length; $ci++) {
                $ch = $txt.Substring($ci, 1)
                if ($ch -ne " ") {
                    $x = [float]14.0 + (($colIdx + $ci) * $charW)
                    $g.DrawString($ch, $font, $brush, $x, $y)
                }
            }
            $brush.Dispose()
        }
        $y += [float]28.0
    }

    $bmp.Save($outputPath, [System.Drawing.Imaging.ImageFormat]::Png)
    $font.Dispose()
    $g.Dispose()
    $bmp.Dispose()
}

$benchmarkCropPath = Join-Path $PSScriptRoot "benchmark_crop.png"
Generate-SyntaxBenchmarkImage $benchmarkCropPath

Write-Output "Successfully generated: $darkPath, $lightPath, $loopDarkPath, $loopLightPath, and $benchmarkCropPath"
