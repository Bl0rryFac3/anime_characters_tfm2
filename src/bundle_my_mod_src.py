#!/usr/bin/env python3
"""Bundle every .rs file under a TFM2 mod src folder into one UTF-8 text file.

Default source (this project's mod):
    C:\\Program Files (x86)\\Steam\\steamapps\\common\\Teamfight Manager2\\mods\\my_mod\\src

Output is written somewhere the game folder does not need admin rights for,
usually the Desktop:
    my_mod_src_bundle.txt

Usage:
    py -3 bundle_my_mod_src.py
    py -3 bundle_my_mod_src.py "D:\\other\\src" "D:\\out\\bundle.txt"
"""

from __future__ import annotations

import sys
from datetime import datetime
from pathlib import Path

DEFAULT_SRC = Path(
    r"C:\Program Files (x86)\Steam\steamapps\common\Teamfight Manager2\mods\my_mod\src"
)
SKIP_DIR_NAMES = {
    "target",
    "node_modules",
    ".git",
    ".arena",
    "__pycache__",
}
BAR = "=" * 80


def windows_known_folder(csidl: int) -> Path | None:
    if sys.platform != "win32":
        return None
    try:
        import ctypes

        buf = ctypes.create_unicode_buffer(32767)
        # SHGetFolderPathW is present on every Windows version this game runs on.
        hr = ctypes.windll.shell32.SHGetFolderPathW(None, csidl, None, 0, buf)
        if hr != 0 or not buf.value:
            return None
        folder = Path(buf.value)
        return folder if folder.is_dir() else None
    except Exception:
        return None


def desktop_dir() -> Path | None:
    known = windows_known_folder(0x0010)  # CSIDL_DESKTOPDIRECTORY
    if known is not None:
        return known
    home = Path.home()
    candidates = [
        home / "Desktop",
        home / "OneDrive" / "Desktop",
        home / "OneDrive - Personal" / "Desktop",
        home / "Documents",
    ]
    for candidate in candidates:
        if candidate.is_dir():
            return candidate
    return home if home.is_dir() else None


def script_dir() -> Path:
    try:
        return Path(__file__).resolve().parent
    except NameError:
        return Path.cwd()


def default_output_path() -> Path:
    """Prefer the folder the user launched from, then Desktop.

    Writing next to the script fails when that folder is under Program Files,
    so callers still have the Desktop fallback inside write_bundle().
    """
    here = script_dir() / "my_mod_src_bundle.txt"
    try:
        here.parent.mkdir(parents=True, exist_ok=True)
        probe = here.with_suffix(".write_probe")
        probe.write_text("ok", encoding="utf-8")
        probe.unlink()
        return here
    except OSError:
        pass
    folder = desktop_dir() or Path.cwd()
    return folder / "my_mod_src_bundle.txt"


def iter_rs_files(src: Path) -> list[Path]:
    found: list[Path] = []
    for path in src.rglob("*"):
        if not path.is_file():
            continue
        if path.suffix.lower() != ".rs":
            continue
        if any(part in SKIP_DIR_NAMES for part in path.relative_to(src).parts):
            continue
        found.append(path)
    found.sort(key=lambda p: p.relative_to(src).as_posix().lower())
    return found


def read_text(path: Path) -> str:
    data = path.read_bytes()
    if data.startswith(b"\xef\xbb\xbf"):
        data = data[3:]
    try:
        return data.decode("utf-8")
    except UnicodeDecodeError:
        return data.decode("cp1252", errors="replace")


def rel_posix(src: Path, path: Path) -> str:
    return path.relative_to(src).as_posix()


def build_bundle(src: Path, files: list[Path]) -> str:
    now = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    lines: list[str] = [
        "TFM2 my_mod src bundle",
        f"Source: {src}",
        f"Generated: {now}",
        f"File count: {len(files)}",
        "",
        "INDEX",
    ]
    for path in files:
        rel = rel_posix(src, path)
        lines.append(f"- {rel} ({path.stat().st_size} bytes)")
    lines.append("")
    chunks: list[str] = ["\n".join(lines), ""]
    for path in files:
        rel = rel_posix(src, path)
        text = read_text(path)
        header = "\n".join(
            [
                BAR,
                f"FILE: {rel}",
                f"FULL: {path}",
                f"BYTES: {path.stat().st_size}",
                BAR,
            ]
        )
        footer = "\n".join(
            [
                BAR,
                f"END FILE: {rel}",
                BAR,
                "",
            ]
        )
        body = text if text.endswith("\n") else text + "\n"
        chunks.append(header + "\n" + body + footer)
    return "\n".join(chunks).rstrip() + "\n"


def write_bundle(out: Path, text: str) -> None:
    out.parent.mkdir(parents=True, exist_ok=True)
    # utf-8-sig so Notepad on Windows shows the file correctly.
    out.write_bytes(b"\xef\xbb\xbf" + text.encode("utf-8"))


def main(argv: list[str]) -> int:
    src = Path(argv[1]).expanduser() if len(argv) > 1 else DEFAULT_SRC
    out = Path(argv[2]).expanduser() if len(argv) > 2 else default_output_path()

    print(f"Source: {src}")
    if not src.is_dir():
        print("ERROR: source folder not found.")
        print("Check that Teamfight Manager 2 is installed in the default Steam folder,")
        print("or pass the src path as the first argument.")
        return 1

    files = iter_rs_files(src)
    text = build_bundle(src, files)
    try:
        write_bundle(out, text)
    except OSError as exc:
        fallback = Path.home() / "my_mod_src_bundle.txt"
        print(f"Could not write {out}: {exc}")
        if fallback.resolve() == out.resolve():
            print("ERROR: no writable output location.")
            return 1
        print(f"Trying fallback: {fallback}")
        try:
            write_bundle(fallback, text)
        except OSError as exc2:
            print(f"ERROR: could not write fallback either: {exc2}")
            return 1
        out = fallback

    print(f"Wrote {len(files)} .rs file(s) to:")
    print(out)
    if not files:
        print("WARNING: the folder exists, but no .rs files were found.")
        return 2
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main(sys.argv))
    except BrokenPipeError:
        raise SystemExit(0)
    except Exception as exc:  # last-resort so a double-click window still explains itself
        print(f"ERROR: {type(exc).__name__}: {exc}")
        raise SystemExit(1)
