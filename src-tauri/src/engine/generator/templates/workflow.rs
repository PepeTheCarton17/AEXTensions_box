/// Plantilla del workflow de GitHub Actions.
/// `aext-managed-workflow` permite migrar repos generados por versiones
/// anteriores de AEXT sin sobrescribir workflows escritos por el usuario.

macro_rules! workflow_yml_fmt {
    () => { r#"# aext-managed-workflow
name: Build and Publish Nekoyomi Extension

on:
  push:
  workflow_dispatch:

concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  # No cancelar runs: cada push de AEXT debe compilarse y verificarse.
  cancel-in-progress: false

permissions:
  contents: write
  pages: write
  id-token: write

jobs:
  build:
    name: Build APK and Update Repo
    runs-on: ubuntu-latest
    timeout-minutes: 30
    steps:
      - name: Checkout code
        uses: actions/checkout@v4

      - name: Set up JDK 21
        uses: actions/setup-java@v5
        with:
          java-version: '21'
          distribution: 'temurin'

      - name: Setup Gradle
        uses: gradle/actions/setup-gradle@v3
        with:
          gradle-version: 9.5.1

      - name: Set up Python
        uses: actions/setup-python@v5
        with:
          python-version: '3.11'

      # Un commit de AEXT puede contener varias extensiones del mismo repo.
      # Se compila módulo por módulo para saber versión, tiempo, warnings y
      # errores de cada una, y se escribe un reporte que AEXT puede leer.
      - name: Build modules with report
        shell: bash
        run: |
          set -uo pipefail
          mkdir -p build-logs aext-report repo_dist
          : > aext-report/build-summary.tsv
          : > aext-report/warnings.tsv
          started_all=$(date +%s)
          failed=0
          module_count=0
          while IFS= read -r gradle_file; do
            module_dir=$(dirname "$gradle_file")
            module_path=${module_dir//\//:}
            module_name=$(basename "$module_dir")
            lang=$(basename "$(dirname "$module_dir")")
            pkg=$(grep -m1 -oP 'applicationId\s*=\s*"\K[^"]+' "$gradle_file" || true)
            version=$(grep -m1 -oP 'versionName\s*=\s*"\K[^"]+' "$gradle_file" || true)
            code=$(grep -m1 -oP 'versionCode\s*=\s*\K[0-9]+' "$gradle_file" || true)
            log_file="build-logs/${lang}-${module_name}.log"
            module_started=$(date +%s)
            echo "== Building :${module_path} (${pkg:-unknown} v${version:-?})"
            gradle ":${module_path}:assembleRelease" --stacktrace --console=plain 2>&1 | tee "$log_file"
            status=${PIPESTATUS[0]}
            seconds=$(( $(date +%s) - module_started ))
            module_count=$(( module_count + 1 ))
            warnings=$(grep -c -E '^w:|warning:' "$log_file" || true)
            errors=$(grep -c -E '^e:|^error:|FAILURE:' "$log_file" || true)
            if [ "$status" -eq 0 ]; then result=success; else result=failure; failed=$(( failed + 1 )); fi
            printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
              "$lang" "$module_name" "$pkg" "$version" "$code" "$result" "$seconds" "$warnings" "$errors" >> aext-report/build-summary.tsv
            grep -E '^w:|warning:' "$log_file" | head -n 20 | while IFS= read -r line; do
              printf '%s\t%s\n' "${lang}/${module_name}" "$line" >> aext-report/warnings.tsv
            done
            if [ "$status" -ne 0 ]; then
              echo "::error title=Build failed:::${module_path}: $(grep -m1 -E '^e:|^error:|FAILURE:' "$log_file" || true)"
            fi
          done < <(find src -type f -name build.gradle.kts | sort)
          total=$(( $(date +%s) - started_all ))
          printf 'modules=%s failed=%s seconds=%s\n' "$module_count" "$failed" "$total" | tee aext-report/build-totals.txt
          if [ "$module_count" -eq 0 ]; then
            echo "::error::AEXT no encontró módulos Gradle en src/"
            exit 1
          fi
          if [ "$failed" -gt 0 ]; then
            echo "::error::${failed} módulo(s) fallaron; no se publicará un índice incompleto"
            exit 1
          fi

      - name: Generate Repo Index
        run: |
          python3 .github/scripts/generate-repo-index.py --out repo_dist

      - name: Write AEXT build report and summary
        shell: bash
        run: |
          python3 - <<'PY'
          import json
          import os
          from datetime import datetime, timezone
          from pathlib import Path

          report_dir = Path('aext-report')
          out = Path('repo_dist')
          out.mkdir(parents=True, exist_ok=True)

          rows = []
          for line in (report_dir / 'build-summary.tsv').read_text().splitlines():
              parts = line.split('\t')
              if len(parts) < 9:
                  continue
              lang, module, pkg, version, code, result, seconds, warnings, errors = parts[:9]
              rows.append({
                  'module': f'{lang}/{module}',
                  'pkg': pkg,
                  'version': version,
                  'code': int(code or 0),
                  'status': result,
                  'seconds': int(seconds or 0),
                  'warnings': int(warnings or 0),
                  'errors': int(errors or 0),
              })

          # `.aext-report.json` lo genera AEXT por módulo con las fuentes que
          # no se pudieron verificar en el análisis del sitio.
          skipped_sources = []
          for path in sorted(Path('src').rglob('.aext-report.json')):
              try:
                  data = json.loads(path.read_text())
              except Exception:
                  continue
              for warning in data.get('warnings', []):
                  skipped_sources.append({'module': path.parent.name, 'detail': warning})

          totals_file = report_dir / 'build-totals.txt'
          totals_text = totals_file.read_text().strip() if totals_file.exists() else ''
          totals = dict(item.split('=', 1) for item in totals_text.split() if '=' in item)
          report = {
              'generated_at': datetime.now(timezone.utc).isoformat(),
              'modules': rows,
              'skipped_sources': skipped_sources,
              'totals': {
                  'modules': int(totals.get('modules', 0)),
                  'failed': int(totals.get('failed', 0)),
                  'seconds': int(totals.get('seconds', 0)),
              },
          }
          (out / 'build-report.json').write_text(
              json.dumps(report, indent=2, ensure_ascii=False) + '\n'
          )

          summary = os.environ.get('GITHUB_STEP_SUMMARY')
          if summary:
              with open(summary, 'a', encoding='utf-8') as handle:
                  handle.write('## AEXT build report\n\n')
                  handle.write(
                      f"Modules: {report['totals']['modules']} · Failed: {report['totals']['failed']} "
                      f"· Build time: {report['totals']['seconds']} s\n\n"
                  )
                  handle.write('| Module | Version | Time | Warnings | Errors | Status |\n')
                  handle.write('|---|---|---|---|---|---|\n')
                  for row in rows:
                      handle.write(
                          f"| {row['module']} | {row['version']} ({row['code']}) | "
                          f"{row['seconds']} s | {row['warnings']} | {row['errors']} | {row['status']} |\n"
                      )
                  if skipped_sources:
                      handle.write('\n### Skipped sources\n\n')
                      for item in skipped_sources:
                          handle.write(f"- {item['module']}: {item['detail']}\n")
          PY

      - name: Upload AEXT build report
        if: always()
        uses: actions/upload-artifact@v4
        with:
          name: aext-build-report
          path: |
            build-logs/**
            aext-report/**
            repo_dist/build-report.json
          if-no-files-found: ignore

      - name: Deploy to GitHub Pages
        uses: peaceiris/actions-gh-pages@v4
        with:
          github_token: ${{ secrets.GITHUB_TOKEN }}
          publish_dir: ./repo_dist
          publish_branch: gh-pages
          force_orphan: true
"# };
}
pub const WORKFLOW_YML: &str = workflow_yml_fmt!();
