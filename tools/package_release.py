#!/usr/bin/env python3
"""Bundle the native executables, decoder and ML runtime into a release archive."""
import argparse
import hashlib
import json
from pathlib import Path
import platform
import plistlib
import shutil
import subprocess
import tarfile
import zipfile

from fetch_onnx_runtime import main as fetch_runtime


def run(*args):
    return subprocess.check_output(args, text=True)


def main():
    parser = argparse.ArgumentParser(__doc__)
    parser.add_argument('--binaries', type=Path, required=True)
    parser.add_argument('--ffmpeg', type=Path, required=True)
    parser.add_argument('--destination', type=Path, default=Path('dist'))
    args = parser.parse_args()
    system = platform.system()
    arch = {'amd64': 'x86_64', 'arm64': 'aarch64'}.get(platform.machine().lower(), platform.machine().lower())
    version = json.loads(run('cargo', 'metadata', '--no-deps', '--format-version', '1'))['packages'][0]['version']
    label = 'macos' if system == 'Darwin' else system.lower()
    name = f'opencam-{version}-{label}-{arch}'
    args.destination.mkdir(parents=True, exist_ok=True)
    folder = args.destination / name
    folder.mkdir(exist_ok=False)
    binaries = folder
    if system == 'Darwin':
        contents = folder / 'OpenCam.app' / 'Contents'
        binaries = contents / 'MacOS'
        binaries.mkdir(parents=True)
        with (contents / 'Info.plist').open('wb') as file:
            plistlib.dump({'CFBundleName': 'OpenCam', 'CFBundleDisplayName': 'OpenCam',
                          'CFBundleIdentifier': 'dev.opencam.desktop', 'CFBundleExecutable': 'opencam',
                          'CFBundlePackageType': 'APPL', 'CFBundleShortVersionString': version,
                          'CFBundleVersion': version, 'NSHighResolutionCapable': True,
                          'LSMinimumSystemVersion': '13.0'}, file)
    for executable in ['opencam', 'opencam-probe']:
        filename = executable + ('.exe' if system == 'Windows' else '')
        shutil.copy2(args.binaries / filename, binaries / filename)
    for filename in ['README.md', 'LICENSE']:
        shutil.copy2(filename, folder)
    shutil.copytree('vendor', folder / 'vendor')
    (folder / 'fonts').mkdir()
    shutil.copy2('desktop/fonts/OFL.txt', folder / 'fonts')
    fetch_runtime([str(binaries)])
    libraries = binaries if system != 'Linux' else binaries / 'lib'
    libraries.mkdir(exist_ok=True)
    patterns = {'Linux': 'lib*.so*', 'Darwin': 'lib*.dylib', 'Windows': '*.dll'}
    source = args.ffmpeg / ('bin' if system == 'Windows' else 'lib')
    for library in source.glob(patterns[system]):
        shutil.copy2(library, libraries / library.name, follow_symlinks=False)
    assert any('avcodec' in f.name for f in libraries.iterdir()), 'Decoder library missing'
    notices = folder / 'vendor' / 'ffmpeg'
    if system == 'Windows':
        notices.mkdir()
        for notice in args.ffmpeg.rglob('*.txt'):
            if 'license' in notice.name.lower() or 'readme' in notice.name.lower():
                shutil.copy2(notice, notices / notice.name)
        shutil.copy2(args.ffmpeg / 'opencam-provenance.json', notices)
    else:
        shutil.copytree(args.ffmpeg / 'notices', notices)
    if system == 'Linux':
        for executable in ['opencam', 'opencam-probe']:
            run('patchelf', '--set-rpath', '$ORIGIN/lib', str(binaries / executable))
        for library in libraries.iterdir():
            if not library.is_symlink():
                run('patchelf', '--set-rpath', '$ORIGIN', str(library))
    elif system == 'Darwin':
        for file in binaries.iterdir():
            if file.is_symlink() or file.suffix not in {'.dylib', ''}:
                continue
            dependencies = run('otool', '-L', str(file)).splitlines()[1:]
            if file.suffix == '.dylib':
                dependencies = dependencies[1:]
            for line in dependencies:
                dependency = line.strip().split(' (')[0]
                if dependency.startswith(('/System/', '/usr/lib/')):
                    continue
                basename = Path(dependency).name
                assert (binaries / basename).exists(), f'Unbundled library: {dependency}'
                run('install_name_tool', '-change', dependency, '@loader_path/' + basename, str(file))
            if file.suffix == '.dylib':
                run('install_name_tool', '-id', '@rpath/' + file.name, str(file))
            run('codesign', '--force', '--sign', '-', str(file))
        run('codesign', '--force', '--deep', '--sign', '-', str(folder / 'OpenCam.app'))
    if system == 'Windows':
        archive = args.destination / (name + '.zip')
        with zipfile.ZipFile(archive, 'w', zipfile.ZIP_DEFLATED) as file:
            for path in folder.rglob('*'):
                if path.is_file():
                    file.write(path, path.relative_to(args.destination))
    else:
        archive = args.destination / (name + '.tar.gz')
        with tarfile.open(archive, 'w:gz') as file:
            file.add(folder, arcname=name)
    checksum = hashlib.sha256(archive.read_bytes()).hexdigest()
    archive.with_name(archive.name + '.sha256').write_text(f'{checksum}  {archive.name}\n')
    print(archive)


if __name__ == '__main__':
    main()
