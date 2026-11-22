/// Plantilla del script Python que genera el índice del repo.

macro_rules! repo_index_py_fmt {
    () => { r#"#!/usr/bin/env python3
"""Genera el repositorio de extensiones para Nekoyomi / Aniyomi.

Lee los APK compilados por Gradle y produce:
  - index.json / index.min.json (formato JSON, el canónico)
  - repo.json (con huella de firma)
  - copia de APKs e iconos para descarga directa

No se publica `index.pb`: Nekoyomi puede descargarlo con prioridad y un
protobuf inválido (JSON copiado) rompería el repositorio. Sin `index.pb` y con
`index_v2` en null, los clientes usan `index.min.json`, que sí es válido.
"""

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

DEFAULT_FINGERPRINT = "fb69d41317176e8690fb80115730b31964748aabbf2fd4fc5b830888ae950306"


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as handle:
        while True:
            chunk = handle.read(8192)
            if not chunk:
                break
            h.update(chunk)
    return h.hexdigest()


def repair_zip_and_fingerprint(apk_path: Path) -> str:
    """Intenta leer la huella del certificado del APK con keytool."""
    try:
        result = subprocess.run(
            ["keytool", "-printcert", "-jarfile", str(apk_path)],
            capture_output=True,
            text=True,
            check=False,
        )
        if result.returncode == 0:
            match = re.search(r"SHA256:\s*([A-Fa-f0-9:]+)", result.stdout)
            if match:
                clean = match.group(1).replace(":", "").lower().strip()
                if len(clean) == 64:
                    return clean
    except Exception as error:
        print(f"[!] keytool no disponible: {error}")
    return DEFAULT_FINGERPRINT


def source_id(pkg: str) -> int:
    digest = hashlib.sha256(pkg.encode("utf-8")).hexdigest()
    return int(digest[:15], 16) % (2 ** 62)


def read_module_meta(apk_path: Path) -> dict:
    meta = {
        "pkg": "",
        "lang": "es",
        "slug": "",
        "code": 1,
        "version": "16.1",
        "name": None,
        "baseUrl": "",
    }

    apk_path = apk_path.resolve()
    module = None
    for parent in apk_path.parents:
        gradle = parent / "build.gradle.kts"
        if not gradle.exists():
            continue
        module = parent
        text = gradle.read_text(encoding="utf-8", errors="ignore")

        match = re.search(r'applicationId\s*=\s*"([^"]+)"', text)
        if match:
            meta["pkg"] = match.group(1)
        match = re.search(r"versionCode\s*=\s*(\d+)", text)
        if match:
            meta["code"] = int(match.group(1))
        match = re.search(r'versionName\s*=\s*"([^"]+)"', text)
        if match:
            meta["version"] = match.group(1)
        break

    if module is not None:
        parts = list(module.parts)
        if "src" in parts:
            index = parts.index("src")
            if len(parts) >= index + 3:
                meta["lang"] = parts[index + 1]
                meta["slug"] = parts[index + 2]

        for kotlin in sorted(module.rglob("*.kt")):
            text = kotlin.read_text(encoding="utf-8", errors="ignore")
            match = re.search(r'override\s+val\s+name\s*=\s*"([^"]+)"', text)
            if match and not meta["name"]:
                meta["name"] = match.group(1)
            match = re.search(r'override\s+val\s+baseUrl\s*=\s*"([^"]+)"', text)
            if match and not meta["baseUrl"]:
                meta["baseUrl"] = match.group(1)
            if meta["name"] and meta["baseUrl"]:
                break

    filename = apk_path.name
    match = re.search(
        r"(?:aniyomi|tachiyomi)-([a-zA-Z0-9_-]+)\.([a-zA-Z0-9_.-]+)-v([0-9.]+)(?:-release)?\.apk",
        filename,
    )
    if match:
        if not meta["lang"]:
            meta["lang"] = match.group(1)
        if not meta["slug"]:
            meta["slug"] = match.group(2)
        if not meta["version"]:
            meta["version"] = match.group(3)

    if not meta["pkg"]:
        prefix = "eu.kanade.tachiyomi.animeextension"
        meta["pkg"] = f"{prefix}.{meta['lang']}.{meta['slug'] or 'extension'}"

    if not meta["slug"]:
        meta["slug"] = meta["pkg"].split(".")[-1]

    return meta


def find_icon(apk_path: Path) -> Path | None:
    apk_path = apk_path.resolve()
    for parent in apk_path.parents:
        if (parent / "build.gradle.kts").exists():
            icon = parent / "src" / "main" / "res" / "mipmap-xxxhdpi" / "ic_launcher.png"
            if icon.exists():
                return icon
            icon = parent / "src" / "main" / "res" / "mipmap-xxhdpi" / "ic_launcher.png"
            if icon.exists():
                return icon
            return None
    return None


def main() -> None:
    parser = argparse.ArgumentParser(description="Generador de índice de repositorio Nekoyomi")
    parser.add_argument("--out", default="repo_dist", help="Carpeta de salida publicada en gh-pages")
    parser.add_argument("--repo-name", default="Nekoyomi Extensions", help="Nombre visible del repositorio")
    parser.add_argument("--repo-url", default="https://github.com", help="URL pública del repositorio")
    args = parser.parse_args()

    output_dir = Path(args.out)
    if not output_dir.is_absolute():
        output_dir = ROOT / output_dir
    output_dir.mkdir(parents=True, exist_ok=True)
    (output_dir / "apk").mkdir(parents=True, exist_ok=True)
    (output_dir / "icon").mkdir(parents=True, exist_ok=True)

    # Se busca desde la raíz del repositorio, no desde el directorio actual: así
    # el script funciona tanto en Actions como ejecutado a mano desde cualquier carpeta.
    apk_files = sorted((ROOT / "src").rglob("build/outputs/apk/release/*.apk"))
    print(f"[*] APKs encontrados: {len(apk_files)}")

    extensions = []
    fingerprint = DEFAULT_FINGERPRINT
    generic_icon = output_dir / "icon" / "ic_launcher.png"

    for apk in apk_files:
        meta = read_module_meta(apk)
        filename = apk.name

        shutil.copy2(apk, output_dir / filename)
        shutil.copy2(apk, output_dir / "apk" / filename)

        icon = find_icon(apk)
        if icon is not None:
            shutil.copy2(icon, generic_icon)
            shutil.copy2(icon, output_dir / "icon" / f"{meta['pkg']}.png")

        if fingerprint == DEFAULT_FINGERPRINT:
            fingerprint = repair_zip_and_fingerprint(apk)

        display_name = meta["name"] or meta["slug"]
        extensions.append(
            {
                "name": f"Aniyomi: {display_name}",
                "pkg": meta["pkg"],
                "apk": filename,
                "lang": meta["lang"],
                "code": meta["code"],
                "version": meta["version"],
                "nsfw": 0,
                "hasReadme": 0,
                "hasChangelog": 0,
                "sha256": sha256_file(apk),
                "size": apk.stat().st_size,
                "sources": [
                    {
                        "name": display_name,
                        "lang": meta["lang"],
                        "id": source_id(meta["pkg"]),
                        "baseUrl": meta["baseUrl"],
                    }
                ],
            }
        )
        print(f"  [+] {display_name} v{meta['version']} ({meta['lang']}) -> {filename}")

    index_json = output_dir / "index.json"
    index_min = output_dir / "index.min.json"
    with open(index_json, "w", encoding="utf-8") as handle:
        json.dump(extensions, handle, indent=2, ensure_ascii=False)
    with open(index_min, "w", encoding="utf-8") as handle:
        json.dump(extensions, handle, separators=(",", ":"), ensure_ascii=False)

    # No se escribe index.pb a propósito (ver docstring): un pb inválido hace
    # que los clientes que lo piden primero descarten el repositorio entero.
    stale_pb = output_dir / "index.pb"
    if stale_pb.exists():
        stale_pb.unlink()

    repo_meta = {
        "index_v2": None,
        "meta": {
            "name": args.repo_name,
            "shortName": args.repo_name.lower().replace(" ", ""),
            # GitHub Actions expone "owner/repo"; fuera de Actions se deja el
            # valor de --repo-url si se pasó.
            "website": (
                f"https://github.com/{os.environ['GITHUB_REPOSITORY']}"
                if os.environ.get("GITHUB_REPOSITORY")
                else args.repo_url
            ),
            "signingKeyFingerprint": fingerprint,
        },
    }
    with open(output_dir / "repo.json", "w", encoding="utf-8") as handle:
        json.dump(repo_meta, handle, indent=2, ensure_ascii=False)

    print(f"[+] Índice generado en {output_dir} (fingerprint {fingerprint})")
    if not extensions:
        print("[!] No se generó ningún APK. Revisa el paso 'Build Extension APK'.")


if __name__ == "__main__":
    main()
"# };
}
pub(crate) use repo_index_py_fmt;
pub const REPO_INDEX_PY: &str = repo_index_py_fmt!();

/// Plantilla del README del repositorio generado.
macro_rules! repo_readme_md_fmt {
    () => { r#"# Extensiones Nekoyomi

Repositorio de extensiones para Nekoyomi / Aniyomi.

## Instalación

Añade esta URL como repositorio de extensiones en Nekoyomi o Aniyomi:

{install_url}

## Información

- Repositorio: {html_url}
- El índice de extensiones se publica en la rama `gh-pages` mediante GitHub Actions.

## Descargo de responsabilidad

Las extensiones se distribuyen «tal cual», sin garantía de funcionamiento. El contenido de los sitios web de terceros es responsabilidad de sus respectivos propietarios.

## Licencia

El código de este repositorio se distribuye bajo licencia MIT salvo que se indique lo contrario en un archivo `LICENSE`.
"# };
}
pub(crate) use repo_readme_md_fmt;
