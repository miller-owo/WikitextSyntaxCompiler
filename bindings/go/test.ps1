param(
    [switch]$Release
)

$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
$profile = if ($Release) { 'release' } else { 'debug' }

Push-Location $root
try {
    cargo build -p wikitext-ffi --target x86_64-pc-windows-gnu @(
        if ($Release) { '--release' }
    )
    if ($LASTEXITCODE -ne 0) { throw 'the shared library did not build' }

    $library = Join-Path $root "target\x86_64-pc-windows-gnu\$profile"

    # Windows resolves DLLs next to the test binary first, and Go puts that
    # binary somewhere else entirely.
    Copy-Item -Force (Join-Path $library 'wikitext.dll') $PSScriptRoot

    Push-Location $PSScriptRoot
    try {
        $env:CGO_CFLAGS = "-I$(Join-Path $root 'crates\wikitext-ffi\include')"
        $env:CGO_LDFLAGS = "-L$library -lwikitext"
        go test -count=1 ./...
        if ($LASTEXITCODE -ne 0) { throw 'the Go tests failed' }
    }
    finally {
        Pop-Location
    }
}
finally {
    Pop-Location
}
