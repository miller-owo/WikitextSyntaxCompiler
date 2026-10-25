# Regenerates THIRD-PARTY-NOTICES.md from Cargo.lock.
#
# The table is built from `cargo metadata` and the copyright line each crate ships
# in its LICENSE file inside the cargo registry, so the result tracks Cargo.lock
# rather than anyone's memory. Run it after adding or updating a dependency, then
# check the diff: a new row is a new attribution obligation.
#
# Windows and pwsh 7 only, which matches the binding test scripts. The file it
# writes is the same on any platform; only the generator is Windows specific.

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$registry = Get-ChildItem "$env:USERPROFILE\.cargo\registry\src" -Directory | Select-Object -First 1
if (-not $registry) { throw 'cargo registry source cache is empty; build the workspace once first' }

$metadata = cargo metadata --format-version 1 --all-features --manifest-path (Join-Path $root 'Cargo.toml') | ConvertFrom-Json

$rows = foreach ($package in ($metadata.packages | Where-Object { $_.source } | Sort-Object name)) {
    $dir = Join-Path $registry.FullName ("{0}-{1}" -f $package.name, $package.version)
    $copyright = ''
    if (Test-Path $dir) {
        $files = Get-ChildItem $dir -File |
            Where-Object { $_.Name -match '(?i)^(LICEN[CS]E|COPYING|UNLICENSE)$|^LICEN[CS]E-(MIT|APACHE|UNICODE)$' } |
            Sort-Object { if ($_.Name -match 'MIT') { 0 } else { 1 } }
        foreach ($file in $files) {
            foreach ($line in (Get-Content $file.FullName -TotalCount 20 -ErrorAction SilentlyContinue)) {
                $trimmed = $line.Trim()
                if ($trimmed -match '(?i)^(copyright|\(c\)|©)' -and
                    $trimmed -notmatch '(?i)above copyright notice|shall be included') {
                    $copyright = $trimmed
                    break
                }
            }
            if ($copyright) { break }
        }
    }
    if ($package.name -eq 'unicode-ident') { $copyright = 'Copyright © 1991-2023 Unicode, Inc.' }
    if (-not $copyright) {
        $copyright = if ($package.authors) { 'Authors: ' + ($package.authors -join ', ') } else { 'the crate authors and contributors' }
    }
    [pscustomobject]@{ Crate = $package.name; Version = $package.version; License = $package.license; Copyright = $copyright }
}

function Get-LicenseBody([string]$path, [string]$startAt) {
    $lines = Get-Content $path
    $index = 0
    if ($startAt) { while ($index -lt $lines.Count -and $lines[$index] -notmatch $startAt) { $index++ } }
    return ($lines[$index..($lines.Count - 1)] -join "`n").TrimEnd()
}

$mit = Get-LicenseBody (Join-Path $registry.FullName 'clap-4.6.7\LICENSE-MIT') 'Permission is hereby granted'
$apache = (Get-Content (Join-Path $registry.FullName 'clap-4.6.7\LICENSE-APACHE') -Raw).TrimEnd()
$iscLines = Get-Content (Join-Path $registry.FullName 'libloading-0.9.0\LICENSE')
$isc = ($iscLines[2..($iscLines.Count - 1)] -join "`n").TrimEnd()
$unicode = (Get-Content (Join-Path $registry.FullName 'unicode-ident-1.0.27\LICENSE-UNICODE') -Raw).TrimEnd()
$unlicense = (Get-Content (Join-Path $registry.FullName 'memchr-2.8.3\UNLICENSE') -Raw).TrimEnd()

$out = [System.Collections.Generic.List[string]]::new()
$out.Add('# Third-party notices')
$out.Add('')
$out.Add('This file lists every third-party crate pinned in `Cargo.lock`, with its License and the')
$out.Add('copyright line that License requires us to reproduce. It exists because `LICENSE` covers')
$out.Add("this project's own code (BSD 2-Clause) and says nothing about the crates it links, while")
$out.Add('MIT, ISC and the Unicode License all require their notice to travel with a binary that')
$out.Add('contains them.')
$out.Add('')
$out.Add('**Nothing here is copyleft.** Every entry is MIT, Apache-2.0, ISC, the Unicode License or')
$out.Add('the Unlicense. `RELEASING.md` records the check that keeps it that way.')
$out.Add('')
$out.Add('The table covers the whole resolved graph, including crates that only run at build time or')
$out.Add('expand macros. A proc-macro contributes no code to a binary, but it is still part of the')
$out.Add('source distribution `Cargo.lock` pins, so listing it costs nothing and leaves nothing to')
$out.Add('explain later.')
$out.Add('')
$out.Add('Which entries actually reach a binary: `wikitext` and `wikitext-lua` link nothing at all;')
$out.Add('the CLI adds `clap`; `wikitext-convert` adds `html5ever` (and with it `markup5ever`,')
$out.Add('`tendril`, `string_cache`, `phf` and `xml5ever`), which the C ABI library, the Node addon and')
$out.Add('the WebAssembly module all inherit; the Node addon additionally links `napi`, which reaches')
$out.Add('`libloading`, `futures`, `parking_lot` and `convert_case`; the WebAssembly module adds')
$out.Add('`wasm-bindgen`.')
$out.Add('')
$out.Add('One algorithm is not a dependency and owes nothing: the civil date conversion in')
$out.Add('`crates/wikitext/src/preprocess/functions.rs` follows Howard Hinnant''s public-domain date')
$out.Add('algorithms.')
$out.Add('')
$out.Add('## Crates')
$out.Add('')
$out.Add('| Crate | Version | License | Copyright |')
$out.Add('|---|---|---|---|')
foreach ($row in $rows) {
    $out.Add(('| `{0}` | {1} | {2} | {3} |' -f $row.Crate, $row.Version, $row.License, $row.Copyright))
}
$out.Add('')
$out.Add('## License texts')
$out.Add('')
$out.Add('Where a crate offers a choice (`MIT OR Apache-2.0` and the like), this project takes the')
$out.Add('MIT option unless the crate terms say otherwise; the copyright line for each is above.')
$out.Add('')
foreach ($section in @(
    @{ Title = 'MIT'; Body = $mit },
    @{ Title = 'Apache License 2.0'; Body = $apache },
    @{ Title = 'ISC'; Body = $isc },
    @{ Title = 'Unicode License V3'; Body = $unicode },
    @{ Title = 'The Unlicense'; Body = $unlicense }
)) {
    $out.Add("### $($section.Title)")
    $out.Add('')
    $out.Add('```text')
    $out.Add($section.Body)
    $out.Add('```')
    $out.Add('')
}
$out.Add('## Keeping this file current')
$out.Add('')
$out.Add('The table comes from `cargo metadata` plus the `LICENSE` file each crate ships in the cargo')
$out.Add('registry, so it reflects `Cargo.lock` rather than anyone''s memory. Adding or updating a')
$out.Add('dependency means regenerating it:')
$out.Add('')
$out.Add('```sh')
$out.Add('pwsh -File scripts/third-party-notices.ps1')
$out.Add('```')
$out.Add('')
$out.Add('The five License texts are read from the crates that use them, so they are verbatim rather')
$out.Add('than retyped. If one of those crates is ever dropped, point the generator at another crate')
$out.Add('under the same License.')
$out.Add('')

$target = Join-Path $root 'THIRD-PARTY-NOTICES.md'
[System.IO.File]::WriteAllText($target, ($out -join "`n") + "`n", [System.Text.UTF8Encoding]::new($false))
Write-Host "wrote $target ($($rows.Count) crates)"
