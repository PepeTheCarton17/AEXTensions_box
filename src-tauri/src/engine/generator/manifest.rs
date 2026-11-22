use super::templates::manifest::android_manifest_xml_fmt;

pub(crate) fn generate_android_manifest(pkg_id: &str, class_name: &str, app_label: &str, ext_type: &str) -> String {
    let tag = if ext_type == "manga" {
        "tachiyomi.extension"
    } else {
        "tachiyomi.animeextension"
    };

    format!(
        concat!(android_manifest_xml_fmt!()),
        app_label = app_label,
        class_name = class_name,
        tag = tag,
        pkg_id = pkg_id,
    )
}

#[cfg(test)]
mod manifest_tests {
    use super::generate_android_manifest;

    #[test]
    fn anime_manifest_declares_extension_marker() {
        let manifest = generate_android_manifest(
            "eu.kanade.tachiyomi.animeextension.es.demo",
            "Demo",
            "Demo",
            "anime",
        );

        assert!(manifest.contains(r#"<uses-feature android:name="tachiyomi.animeextension" />"#));
        assert!(manifest.contains(r#"android:name="tachiyomi.animeextension""#));
        assert!(manifest.contains(r#"android:value="true""#));
        assert!(manifest.contains(
            r#"android:value="eu.kanade.tachiyomi.animeextension.es.demo.Demo""#
        ));
    }

    #[test]
    fn manga_manifest_declares_extension_marker() {
        let manifest = generate_android_manifest(
            "eu.kanade.tachiyomi.extension.es.manga",
            "Manga",
            "Manga",
            "manga",
        );

        assert!(manifest.contains(r#"<uses-feature android:name="tachiyomi.extension" />"#));
        assert!(manifest.contains(r#"android:name="tachiyomi.extension""#));
    }
}
