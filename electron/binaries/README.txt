Place platform-specific knotcoind binaries here before building the Electron app.

Expected names (recommended):
- macOS Apple Silicon: knotcoind-aarch64-apple-darwin (or knotcoind)
- Windows x64: knotcoind-x86_64-pc-windows-msvc.exe (or knotcoind.exe)
- Linux x64: knotcoind-x86_64-unknown-linux-gnu (or knotcoind)

The Electron main process will pick the first matching file for your platform.
