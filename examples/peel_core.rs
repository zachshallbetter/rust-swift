// Peel Core capabilities and format detection models in Rust
// To be compiled directly to native Swift via rust-swift compiler.

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Support {
    Supported,
    Conditional,
    Unsupported,
    Experimental,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Zip,
    Tar,
    TarGz,
    TarBz2,
    TarXz,
    TarZst,
    GzipSingle,
    Bzip2Single,
    XzSingle,
    ZstdSingle,
    Rar,
    SevenZ,
    Dmg,
    AppBundle,
    Pdf,
    Exe,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ZipKind {
    Generic,
    Epub,
    Jar,
    Apk,
    Usdz,
    ThreeMf,
    OfficeOpenXml,
    OpenDocument,
    ComicCbz,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Collision {
    Error,
    Overwrite,
    Rename,
    Skip,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EnclosingDirectory {
    WhenNeeded,
    Always,
    Never,
}

pub struct FormatCapabilities {
    pub format: String,
    pub family: String,
    pub probe: Support,
    pub list: Support,
    pub extract_all: Support,
    pub extract_selected: Support,
    pub create: Support,
    pub password: Support,
    pub multipart: Support,
    pub notes: String,
}

pub fn archive_tar_like(name: String) -> FormatCapabilities {
    FormatCapabilities {
        format: name,
        family: String::from("archive"),
        probe: Support::Supported,
        list: Support::Supported,
        extract_all: Support::Supported,
        extract_selected: Support::Conditional,
        create: Support::Unsupported,
        password: Support::Unsupported,
        multipart: Support::Unsupported,
        notes: String::from("ustar/pax via peel-core"),
    }
}

pub fn stream_single(name: String, notes: String) -> FormatCapabilities {
    FormatCapabilities {
        format: name,
        family: String::from("stream"),
        probe: Support::Supported,
        list: Support::Supported,
        extract_all: Support::Supported,
        extract_selected: Support::Unsupported,
        create: Support::Unsupported,
        password: Support::Unsupported,
        multipart: Support::Unsupported,
        notes: notes,
    }
}

pub fn zip_package(name: String, notes: String) -> FormatCapabilities {
    FormatCapabilities {
        format: name,
        family: String::from("package"),
        probe: Support::Supported,
        list: Support::Supported,
        extract_all: Support::Supported,
        extract_selected: Support::Supported,
        create: Support::Unsupported,
        password: Support::Conditional,
        multipart: Support::Unsupported,
        notes: notes,
    }
}

pub fn capabilities_for(format: Format) -> FormatCapabilities {
    match format {
        Format::Zip => FormatCapabilities {
            format: String::from("zip"),
            family: String::from("archive"),
            probe: Support::Supported,
            list: Support::Supported,
            extract_all: Support::Supported,
            extract_selected: Support::Supported,
            create: Support::Unsupported,
            password: Support::Conditional,
            multipart: Support::Unsupported,
            notes: String::from("ZIP/ZIP64 path-safe extract"),
        },
        Format::Tar => archive_tar_like(String::from("tar")),
        Format::TarGz => archive_tar_like(String::from("targz")),
        Format::TarBz2 => archive_tar_like(String::from("tarbz2")),
        Format::TarXz => archive_tar_like(String::from("tarxz")),
        Format::TarZst => archive_tar_like(String::from("tarzst")),
        Format::GzipSingle => stream_single(String::from("gzip"), String::from("single-stream inflate")),
        Format::Bzip2Single => stream_single(String::from("bzip2"), String::from("single-stream inflate")),
        Format::XzSingle => stream_single(String::from("xz"), String::from("single-stream lzma2 inflate")),
        Format::ZstdSingle => stream_single(String::from("zstd"), String::from("single-stream zstd inflate")),
        Format::Rar => FormatCapabilities {
            format: String::from("rar"),
            family: String::from("archive"),
            probe: Support::Supported,
            list: Support::Supported,
            extract_all: Support::Supported,
            extract_selected: Support::Conditional,
            create: Support::Unsupported,
            password: Support::Conditional,
            multipart: Support::Supported,
            notes: String::from("RAR archive support"),
        },
        Format::SevenZ => FormatCapabilities {
            format: String::from("7z"),
            family: String::from("archive"),
            probe: Support::Supported,
            list: Support::Supported,
            extract_all: Support::Supported,
            extract_selected: Support::Conditional,
            create: Support::Unsupported,
            password: Support::Conditional,
            multipart: Support::Unsupported,
            notes: String::from("7z archive support"),
        },
        Format::Dmg => FormatCapabilities {
            format: String::from("dmg"),
            family: String::from("disk_image"),
            probe: Support::Supported,
            list: Support::Conditional,
            extract_all: Support::Conditional,
            extract_selected: Support::Unsupported,
            create: Support::Unsupported,
            password: Support::Unsupported,
            multipart: Support::Unsupported,
            notes: String::from("macOS disk image"),
        },
        Format::AppBundle => FormatCapabilities {
            format: String::from("app"),
            family: String::from("package"),
            probe: Support::Supported,
            list: Support::Supported,
            extract_all: Support::Supported,
            extract_selected: Support::Unsupported,
            create: Support::Unsupported,
            password: Support::Unsupported,
            multipart: Support::Unsupported,
            notes: String::from("directory bundle copy"),
        },
        Format::Pdf => FormatCapabilities {
            format: String::from("pdf"),
            family: String::from("document"),
            probe: Support::Supported,
            list: Support::Experimental,
            extract_all: Support::Experimental,
            extract_selected: Support::Unsupported,
            create: Support::Unsupported,
            password: Support::Unsupported,
            multipart: Support::Unsupported,
            notes: String::from("document inspect-only"),
        },
        Format::Exe => FormatCapabilities {
            format: String::from("exe"),
            family: String::from("document"),
            probe: Support::Supported,
            list: Support::Experimental,
            extract_all: Support::Experimental,
            extract_selected: Support::Unsupported,
            create: Support::Unsupported,
            password: Support::Unsupported,
            multipart: Support::Unsupported,
            notes: String::from("executable inspect-only"),
        },
    }
}

pub fn capabilities_for_zip_kind(kind: ZipKind) -> FormatCapabilities {
    match kind {
        ZipKind::Generic => capabilities_for(Format::Zip),
        ZipKind::Epub => zip_package(String::from("epub"), String::from("ZIP package (OEBPS)")),
        ZipKind::Jar => zip_package(String::from("jar"), String::from("Java JAR/WAR/EAR")),
        ZipKind::Apk => zip_package(String::from("apk"), String::from("Android APK")),
        ZipKind::Usdz => zip_package(String::from("usdz"), String::from("USDZ 3D pack")),
        ZipKind::ThreeMf => zip_package(String::from("3mf"), String::from("3MF 3D printing")),
        ZipKind::OfficeOpenXml => zip_package(String::from("ooxml"), String::from("Office Open XML")),
        ZipKind::OpenDocument => zip_package(String::from("opendocument"), String::from("ODF document")),
        ZipKind::ComicCbz => zip_package(String::from("cbz"), String::from("Comic Book ZIP")),
    }
}

pub fn zip_kind_label(kind: ZipKind) -> String {
    match kind {
        ZipKind::Generic => String::from("zip"),
        ZipKind::Epub => String::from("epub"),
        ZipKind::Jar => String::from("jar"),
        ZipKind::Apk => String::from("apk"),
        ZipKind::Usdz => String::from("usdz"),
        ZipKind::ThreeMf => String::from("3mf"),
        ZipKind::OfficeOpenXml => String::from("ooxml"),
        ZipKind::OpenDocument => String::from("opendocument"),
        ZipKind::ComicCbz => String::from("cbz"),
    }
}

pub fn detect_zip_kind_from_extension(ext: String) -> ZipKind {
    if ext == String::from("epub") {
        ZipKind::Epub
    } else if ext == String::from("jar") {
        ZipKind::Jar
    } else if ext == String::from("war") {
        ZipKind::Jar
    } else if ext == String::from("ear") {
        ZipKind::Jar
    } else if ext == String::from("apk") {
        ZipKind::Apk
    } else if ext == String::from("xapk") {
        ZipKind::Apk
    } else if ext == String::from("usdz") {
        ZipKind::Usdz
    } else if ext == String::from("3mf") {
        ZipKind::ThreeMf
    } else if ext == String::from("docx") {
        ZipKind::OfficeOpenXml
    } else if ext == String::from("xlsx") {
        ZipKind::OfficeOpenXml
    } else if ext == String::from("pptx") {
        ZipKind::OfficeOpenXml
    } else if ext == String::from("odt") {
        ZipKind::OpenDocument
    } else if ext == String::from("ods") {
        ZipKind::OpenDocument
    } else if ext == String::from("odp") {
        ZipKind::OpenDocument
    } else if ext == String::from("cbz") {
        ZipKind::ComicCbz
    } else {
        ZipKind::Generic
    }
}
