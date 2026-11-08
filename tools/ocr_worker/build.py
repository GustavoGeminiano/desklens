"""Build on the target OS. Stage the complete frozen runtime for Tauri resources."""
import argparse
from pathlib import Path
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, default=ROOT / 'build' / 'ocr-runtime')
    parser.add_argument('--model-dir', type=Path, default=ROOT / '.ocr-models')
    parser.add_argument('--prepare-models', action='store_true', help='Explicitly allow model download at build time')
    args = parser.parse_args()
    sys.path.insert(0, str(ROOT / 'scripts' / 'ocr_benchmark'))
    from rapidocr_engine import prepare_models, verify_models, MODELS
    if args.prepare_models:
        prepare_models(args.model_dir)
    verify_models(args.model_dir)
    work = ROOT / 'build' / 'ocr-pyinstaller'
    subprocess.run([sys.executable, '-m', 'PyInstaller', '--noconfirm', '--clean', '--onedir',
                    '--name', 'desklens-ocr', '--distpath', str(work / 'dist'),
                    '--workpath', str(work / 'work'), '--specpath', str(work),
                    '--paths', str(ROOT / 'scripts' / 'ocr_benchmark'),
                    '--collect-all', 'rapidocr', '--collect-all', 'onnxruntime',
                    '--copy-metadata', 'rapidocr', '--copy-metadata', 'onnxruntime',
                    str(ROOT / 'tools' / 'ocr_worker' / 'worker.py')], check=True)
    runtime = work / 'dist' / 'desklens-ocr'
    models = runtime / 'models'
    models.mkdir(exist_ok=True)
    for model in MODELS.values():
        shutil.copy2(args.model_dir / model['name'], models / model['name'])
    # copytree dirs_exist_ok is deliberate: only generated staging is replaced.
    args.output.mkdir(parents=True, exist_ok=True)
    shutil.copytree(runtime, args.output, dirs_exist_ok=True)
    executable = args.output / ('desklens-ocr.exe' if sys.platform == 'win32' else 'desklens-ocr')
    print(executable.resolve())


if __name__ == '__main__':
    main()
