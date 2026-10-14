import XCTest
@testable import PeelCoreKit

final class PeelCoreKitTests: XCTestCase {
    func testZipCapabilities() {
        let caps = capabilities_for(.Zip)
        XCTAssertEqual(caps.format, "zip")
        XCTAssertEqual(caps.family, "archive")
        XCTAssertEqual(caps.probe, .Supported)
        XCTAssertEqual(caps.list, .Supported)
        XCTAssertEqual(caps.extract_all, .Supported)
        XCTAssertEqual(caps.extract_selected, .Supported)
        XCTAssertEqual(caps.create, .Unsupported)
        XCTAssertEqual(caps.password, .Conditional)
        XCTAssertEqual(caps.multipart, .Unsupported)
    }

    func testTarCapabilities() {
        let caps = capabilities_for(.Tar)
        XCTAssertEqual(caps.format, "tar")
        XCTAssertEqual(caps.family, "archive")
        XCTAssertEqual(caps.probe, .Supported)
        XCTAssertEqual(caps.list, .Supported)
        XCTAssertEqual(caps.extract_all, .Supported)
        XCTAssertEqual(caps.extract_selected, .Conditional)
        XCTAssertEqual(caps.create, .Unsupported)
    }

    func testRarCapabilities() {
        let caps = capabilities_for(.Rar)
        XCTAssertEqual(caps.format, "rar")
        XCTAssertEqual(caps.family, "archive")
        XCTAssertEqual(caps.probe, .Supported)
        XCTAssertEqual(caps.list, .Supported)
        XCTAssertEqual(caps.extract_all, .Supported)
        XCTAssertEqual(caps.extract_selected, .Conditional)
        XCTAssertEqual(caps.multipart, .Supported)
    }

    func testSevenZCapabilities() {
        let caps = capabilities_for(.SevenZ)
        XCTAssertEqual(caps.format, "7z")
        XCTAssertEqual(caps.family, "archive")
        XCTAssertEqual(caps.probe, .Supported)
        XCTAssertEqual(caps.extract_all, .Supported)
        XCTAssertEqual(caps.multipart, .Unsupported)
    }

    func testDmgCapabilities() {
        let caps = capabilities_for(.Dmg)
        XCTAssertEqual(caps.format, "dmg")
        XCTAssertEqual(caps.family, "disk_image")
        XCTAssertEqual(caps.probe, .Supported)
        XCTAssertEqual(caps.list, .Conditional)
        XCTAssertEqual(caps.extract_all, .Conditional)
        XCTAssertEqual(caps.extract_selected, .Unsupported)
    }

    func testZipPackages() {
        let epub = capabilities_for_zip_kind(.Epub)
        XCTAssertEqual(epub.format, "epub")
        XCTAssertEqual(epub.family, "package")
        XCTAssertEqual(epub.extract_selected, .Supported)

        let apk = capabilities_for_zip_kind(.Apk)
        XCTAssertEqual(apk.format, "apk")
        XCTAssertEqual(apk.family, "package")

        let usdz = capabilities_for_zip_kind(.Usdz)
        XCTAssertEqual(usdz.format, "usdz")
        XCTAssertEqual(usdz.family, "package")

        let ooxml = capabilities_for_zip_kind(.OfficeOpenXml)
        XCTAssertEqual(ooxml.format, "ooxml")
        XCTAssertEqual(ooxml.family, "package")
    }

    func testZipKindLabels() {
        XCTAssertEqual(zip_kind_label(.Generic), "zip")
        XCTAssertEqual(zip_kind_label(.Epub), "epub")
        XCTAssertEqual(zip_kind_label(.Jar), "jar")
        XCTAssertEqual(zip_kind_label(.Apk), "apk")
        XCTAssertEqual(zip_kind_label(.Usdz), "usdz")
        XCTAssertEqual(zip_kind_label(.ThreeMf), "3mf")
        XCTAssertEqual(zip_kind_label(.OfficeOpenXml), "ooxml")
        XCTAssertEqual(zip_kind_label(.OpenDocument), "opendocument")
        XCTAssertEqual(zip_kind_label(.ComicCbz), "cbz")
    }

    func testExtensionDetection() {
        XCTAssertEqual(detect_zip_kind_from_extension("epub"), .Epub)
        XCTAssertEqual(detect_zip_kind_from_extension("jar"), .Jar)
        XCTAssertEqual(detect_zip_kind_from_extension("war"), .Jar)
        XCTAssertEqual(detect_zip_kind_from_extension("ear"), .Jar)
        XCTAssertEqual(detect_zip_kind_from_extension("apk"), .Apk)
        XCTAssertEqual(detect_zip_kind_from_extension("xapk"), .Apk)
        XCTAssertEqual(detect_zip_kind_from_extension("usdz"), .Usdz)
        XCTAssertEqual(detect_zip_kind_from_extension("3mf"), .ThreeMf)
        XCTAssertEqual(detect_zip_kind_from_extension("docx"), .OfficeOpenXml)
        XCTAssertEqual(detect_zip_kind_from_extension("xlsx"), .OfficeOpenXml)
        XCTAssertEqual(detect_zip_kind_from_extension("pptx"), .OfficeOpenXml)
        XCTAssertEqual(detect_zip_kind_from_extension("odt"), .OpenDocument)
        XCTAssertEqual(detect_zip_kind_from_extension("ods"), .OpenDocument)
        XCTAssertEqual(detect_zip_kind_from_extension("odp"), .OpenDocument)
        XCTAssertEqual(detect_zip_kind_from_extension("cbz"), .ComicCbz)
        XCTAssertEqual(detect_zip_kind_from_extension("tar"), .Generic)
        XCTAssertEqual(detect_zip_kind_from_extension("unknown"), .Generic)
    }
}
