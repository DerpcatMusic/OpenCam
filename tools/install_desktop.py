#!/usr/bin/env python3
"""Install a Linux release for the current user, including menu and Desktop launchers."""
import argparse
import hashlib
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile


def digest(path):
    with path.open('rb') as file:
        return hashlib.file_digest(file, 'sha256').hexdigest()


def replace_link(path, target):
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.is_dir() and not path.is_symlink():
        raise RuntimeError(f'{path} is a directory; keep it and choose another installation location')
    temporary = path.with_name(path.name + f'.new-{os.getpid()}')
    temporary.symlink_to(target)
    temporary.replace(path)


def main():
    parser = argparse.ArgumentParser(__doc__)
    parser.add_argument('--package', type=Path, default=Path(__file__).resolve().parent)
    parser.add_argument('--desktop', action='store_true', help='Also create a Desktop shortcut')
    args = parser.parse_args()
    if not sys.platform.startswith('linux'):
        parser.error('This installer is for the Linux release')
    package = args.package.resolve()
    executable = package / 'opencam'
    version_text = subprocess.check_output([str(executable), '--version'], text=True, timeout=10).strip()
    match = re.fullmatch(r'OpenCam (\d+\.\d+\.\d+)', version_text)
    if not match or not (package / 'opencam-probe').is_file():
        parser.error('Choose an extracted OpenCam Linux release with both executables')
    version = match.group(1)
    home = Path.home()
    app = home / '.local/share/opencam'
    releases = app / 'releases'
    releases.mkdir(parents=True, exist_ok=True)
    destination = releases / f'v{version}'
    if destination.exists() and digest(destination / 'opencam') != digest(executable):
        destination = releases / f'v{version}-{digest(executable)[:12]}'
    if not destination.exists():
        temporary = Path(tempfile.mkdtemp(prefix='.install-', dir=releases))
        try:
            shutil.copytree(package, temporary, symlinks=True, dirs_exist_ok=True)
            subprocess.run([str(temporary / 'opencam'), '--version'], check=True, capture_output=True)
            temporary.rename(destination)
        finally:
            if temporary.exists():
                shutil.rmtree(temporary)
    replace_link(app / 'current', destination.relative_to(app))
    for name in ['opencam', 'opencam-probe']:
        replace_link(home / '.local/bin' / name, app / 'current' / name)
    icon = 'camera-web'
    if (package / 'opencam.svg').is_file():
        icons = home / '.local/share/icons/hicolor/scalable/apps'
        icons.mkdir(parents=True, exist_ok=True)
        shutil.copy2(package / 'opencam.svg', icons / 'dev.opencam.OpenCam.svg')
        icon = 'dev.opencam.OpenCam'
    launch = str(home / '.local/bin/opencam').replace('\\', '\\\\').replace('"', '\\"').replace('%', '%%')
    entry = '\n'.join([
        '[Desktop Entry]', 'Type=Application', 'Name=OpenCam',
        'Comment=Use your Android phone as a USB or Wi-Fi webcam',
        f'Exec="{launch}"', f'Icon={icon}', 'Terminal=false',
        'Categories=AudioVideo;Video;', 'StartupWMClass=dev.opencam.OpenCam', '',
    ])
    applications = home / '.local/share/applications'
    applications.mkdir(parents=True, exist_ok=True)
    menu = applications / 'dev.opencam.OpenCam.desktop'
    menu.write_text(entry)
    if shutil.which('update-desktop-database'):
        subprocess.run(['update-desktop-database', str(applications)], check=True)
    if args.desktop:
        desktop = home / 'Desktop'
        if shutil.which('xdg-user-dir'):
            configured = subprocess.check_output(['xdg-user-dir', 'DESKTOP'], text=True).strip()
            if configured and Path(configured) != home:
                desktop = Path(configured)
        desktop.mkdir(parents=True, exist_ok=True)
        shortcut = desktop / 'OpenCam.desktop'
        shortcut.write_text(entry)
        shortcut.chmod(0o755)
        if shutil.which('gio'):
            subprocess.run(['gio', 'set', str(shortcut), 'metadata::trusted', 'true'], capture_output=True)
        print(f'Desktop shortcut: {shortcut}')
    print(f'Installed {version_text}: {destination}')
    print(f'Applications menu: {menu}')
    print(f'Launch: {home / ".local/bin/opencam"}')


if __name__ == '__main__':
    main()
