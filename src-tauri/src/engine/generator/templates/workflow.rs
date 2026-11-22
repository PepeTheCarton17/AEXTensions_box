/// Plantilla del workflow de GitHub Actions.

macro_rules! workflow_yml_fmt {
    () => { r#"name: Build and Publish Nekoyomi Extension

on:
  push:
  workflow_dispatch:

concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  # No cancelar runs: cada commit de AEXT debe compilarse y verificarse. Con
  # `true`, los commits rápidos cancelaban el run útil y no había generación.
  cancel-in-progress: false

permissions:
  contents: write
  pages: write
  id-token: write

jobs:
  build:
    name: Build APK and Update Repo
    runs-on: ubuntu-latest
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

      - name: Build Extension APK
        run: gradle assembleRelease --stacktrace

      - name: Set up Python
        uses: actions/setup-python@v5
        with:
          python-version: '3.11'

      - name: Generate Repo Index
        run: |
          python3 .github/scripts/generate-repo-index.py --out repo_dist

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
