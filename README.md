# CUMS - Custom Unified Music and Sounds

[![GitHub release](https://img.shields.io/github/v/release/valkyaha/CUMS?style=flat-square)](https://github.com/valkyaha/CUMS/releases/latest)
[![Downloads](https://img.shields.io/github/downloads/valkyaha/CUMS/total?style=flat-square)](https://github.com/valkyaha/CUMS/releases)
[![Build](https://img.shields.io/github/actions/workflow/status/valkyaha/CUMS/build.yml?style=flat-square)](https://github.com/valkyaha/CUMS/actions)
[![License](https://img.shields.io/github/license/valkyaha/CUMS?style=flat-square)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows-blue?style=flat-square)]()

Audio modding tool for FromSoftware games.

## Supported Games

| Game | Format | Codec | Encryption |
|------|--------|-------|------------|
| Sekiro: Shadows Die Twice | FSB5 | Vorbis | AES-256 |
| Dark Souls 3 | FSB5 | Vorbis | AES-256 |
| Dark Souls 2: Scholar of the First Sin | FSB5 | Vorbis | None |
| Dark Souls 1 | FSB4 | MP3 | None |

## Download

[![Download](https://img.shields.io/badge/Download-Latest%20Release-brightgreen?style=for-the-badge)](https://github.com/valkyaha/CUMS/releases/latest)

- **CUMS-full.zip** - Includes FFmpeg for automatic audio conversion (~80MB)
- **CUMS-lite.zip** - Smaller download, requires FFmpeg separately (~5MB)

## Usage

1. Run `cums.exe`
2. Drag & drop FSB files or click **Open Files** / **Open Folder**
3. Click a sound to play it
4. Click **Replace** to swap with your own audio (WAV/MP3/OGG/FLAC)
5. Adjust volume/pitch/speed if needed
6. Click **Save** to create the modified FSB

## Dependencies

### For Users (Pre-built)

The release includes everything needed:
- `cums.exe` - Main application
- `ffmpeg.exe` - Audio conversion (optional but recommended)
- `lib/fmod/` - FMOD encoding tools

If using CUMS-lite, install FFmpeg from https://ffmpeg.org/download.html or place `ffmpeg.exe` next to `cums.exe`.

### For Developers (Building from Source)

**Requirements:**
- Rust 1.70+ (https://rustup.rs)
- FMOD tools in `lib/fmod/` directory (included in repo)

**Build:**
```bash
cargo build --release -p cums-gui
```

The executable will be at `target/release/cums.exe`.

## File Structure

```
CUMS/
├── cums.exe              # Main application
├── ffmpeg.exe            # Audio converter (optional)
└── lib/
    └── fmod/
        ├── fsbankcl.exe  # FMOD FSB encoder
        ├── fmod.dll
        ├── fsbank.dll
        ├── libfsbvorbis.dll
        ├── libogg.dll
        └── libvorbis.dll
```

## Installation Notes

- **Sekiro/DS3**: Place modified FSB files in `mods/sound/` folder (requires ModEngine2)
- **DS2 SotFS**: Replace files directly in `Game/sound/` (backup originals first)
- **DS1**: Replace files in `sound/` folder

## License

MIT
