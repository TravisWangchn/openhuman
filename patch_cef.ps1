$path = 'C:\Users\bigdata\.cargo\registry\src\index.crates.io-1949cf8c6b5b557f\cef-dll-sys-146.4.1+146.0.9\build.rs'
$content = [System.IO.File]::ReadAllText($path)
$content = $content.Replace('clang-cl', 'cl.exe')
[System.IO.File]::WriteAllText($path, $content)
Write-Output "Patched CEF build.rs: clang-cl -> cl.exe"
