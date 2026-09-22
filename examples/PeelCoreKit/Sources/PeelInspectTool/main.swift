import Foundation
import PeelCoreKit

print("=== PeelCoreKit Native Swift Inspection (Zero FFI) ===")

let formats: [Format] = [
    .Zip, .Tar, .TarGz, .TarBz2, .TarXz, .TarZst,
    .GzipSingle, .Bzip2Single, .XzSingle, .ZstdSingle,
    .Rar, .SevenZ, .Dmg, .AppBundle, .Pdf, .Exe
]

print("\n--- Core Formats (\(formats.count) formats) ---")
for fmt in formats {
    let caps = capabilities_for(fmt)
    print("[\(caps.format)] family: \(caps.family), probe: \(caps.probe), list: \(caps.list), extract: \(caps.extract_all), notes: \(caps.notes)")
}

let zipKinds: [ZipKind] = [
    .Generic, .Epub, .Jar, .Apk, .Usdz, .ThreeMf, .OfficeOpenXml, .OpenDocument, .ComicCbz
]

print("\n--- ZIP-Shaped Packages (\(zipKinds.count) kinds) ---")
for kind in zipKinds {
    let caps = capabilities_for_zip_kind(kind)
    let label = zip_kind_label(kind)
    print("[\(label)] format: \(caps.format), family: \(caps.family), extract_selected: \(caps.extract_selected), notes: \(caps.notes)")
}

print("\n--- Path Extension Detection ---")
let testExtensions = ["epub", "apk", "usdz", "3mf", "docx", "odt", "cbz", "jar", "tar", "unknown"]
for ext in testExtensions {
    let detected = detect_zip_kind_from_extension(ext)
    print("extension: .\(ext) -> ZipKind.\(detected.rawValue)")
}

print("\nVerification: 100% Native Swift Value Types & Logic executed successfully.")
