/// Plantilla del AndroidManifest.xml generado.

macro_rules! android_manifest_xml_fmt {
    () => { r#"<?xml version="1.0" encoding="utf-8"?>
<manifest xmlns:android="http://schemas.android.com/apk/res/android">

    <!-- Marcador usado por Nekoyomi/Aniyomi para distinguir extensiones de apps normales. -->
    <uses-feature android:name="{tag}" />

    <application
        android:label="{app_label}"
        android:icon="@mipmap/ic_launcher"
        android:allowBackup="true"
        android:supportsRtl="true">
        <meta-data
            android:name="{tag}"
            android:value="true" />
        <meta-data
            android:name="{tag}.class"
            android:value="{pkg_id}.{class_name}" />
        <meta-data
            android:name="{tag}.hasSettings"
            android:value="true" />
        <meta-data
            android:name="{tag}.nsfw"
            android:value="0" />
    </application>
</manifest>
"# };
}
pub(crate) use android_manifest_xml_fmt;
