import SwiftUI
import PeelCoreKit

@main
struct PeelInspectorApp: App {
    var body: some Scene {
        WindowGroup {
            PeelInspectorView()
        }
    }
}

struct PeelInspectorView: View {
    @State private var selectedTab: Int = 0
    @State private var searchExtension: String = "cbz"
    @State private var detectedKind: ZipKind = .ComicCbz

    private let formats: [Format] = [
        .Zip, .Tar, .TarGz, .TarBz2, .TarXz, .TarZst,
        .GzipSingle, .Bzip2Single, .XzSingle, .ZstdSingle,
        .Rar, .SevenZ, .Dmg, .AppBundle, .Pdf, .Exe
    ]

    private let zipKinds: [ZipKind] = [
        .Generic, .Epub, .Jar, .Apk, .Usdz, .ThreeMf, .OfficeOpenXml, .OpenDocument, .ComicCbz
    ]

    var body: some View {
        VStack(spacing: 0) {
            // Header
            VStack(alignment: .leading, spacing: 4) {
                HStack {
                    Text("Peel Format Matrix")
                        .font(.title)
                        .bold()
                    Spacer()
                    Text("Zero FFI • Compiled from Rust")
                        .font(.caption)
                        .padding(.horizontal, 8)
                        .padding(.vertical, 4)
                        .background(Color.green.opacity(0.15))
                        .foregroundColor(.green)
                        .cornerRadius(6)
                }
                Text("Native Swift models & functions compiled by rust-swift from peel-core")
                    .font(.subheadline)
                    .foregroundColor(.secondary)
            }
            .padding()
            .background(Color(NSColor.windowBackgroundColor))

            Divider()

            // Extension Probe Interactive Bar
            HStack(spacing: 12) {
                Text("Inspect Extension:")
                    .font(.headline)
                TextField("ext (e.g. epub, docx, cbz)", text: $searchExtension)
                    .textFieldStyle(.roundedBorder)
                    .frame(width: 140)
                    .onChange(of: searchExtension) { _, newValue in
                        detectedKind = detect_zip_kind_from_extension(newValue.lowercased())
                    }

                let caps = capabilities_for_zip_kind(detectedKind)
                VStack(alignment: .leading, spacing: 2) {
                    HStack {
                        Text("Detected: \(zip_kind_label(detectedKind).uppercased())")
                            .bold()
                        Text("(\(caps.family))")
                            .foregroundColor(.secondary)
                    }
                    Text(caps.notes)
                        .font(.caption)
                        .foregroundColor(.secondary)
                }
                Spacer()
            }
            .padding(.horizontal)
            .padding(.vertical, 10)
            .background(Color(NSColor.controlBackgroundColor))

            Divider()

            // Matrix Table
            ScrollView {
                VStack(spacing: 8) {
                    ForEach(formats, id: \.rawValue) { fmt in
                        let c = capabilities_for(fmt)
                        FormatRowView(caps: c)
                    }
                }
                .padding()
            }
        }
        .frame(minWidth: 680, minHeight: 520)
    }
}

struct FormatRowView: View {
    let caps: FormatCapabilities

    var body: some View {
        HStack(alignment: .center, spacing: 12) {
            VStack(alignment: .leading, spacing: 2) {
                HStack {
                    Text(caps.format.uppercased())
                        .font(.system(.body, design: .monospaced))
                        .bold()
                    Text(caps.family)
                        .font(.caption2)
                        .padding(.horizontal, 6)
                        .padding(.vertical, 2)
                        .background(Color.blue.opacity(0.12))
                        .foregroundColor(.blue)
                        .cornerRadius(4)
                }
                Text(caps.notes)
                    .font(.caption)
                    .foregroundColor(.secondary)
            }
            .frame(width: 260, alignment: .leading)

            Spacer()

            HStack(spacing: 8) {
                SupportBadge(label: "Probe", support: caps.probe)
                SupportBadge(label: "List", support: caps.list)
                SupportBadge(label: "Extract", support: caps.extract_all)
                SupportBadge(label: "Select", support: caps.extract_selected)
                SupportBadge(label: "Passwd", support: caps.password)
            }
        }
        .padding(.vertical, 6)
        .padding(.horizontal, 10)
        .background(Color(NSColor.alternatingContentBackgroundColors[1]))
        .cornerRadius(6)
    }
}

struct SupportBadge: View {
    let label: String
    let support: Support

    var body: some View {
        VStack(spacing: 2) {
            Text(label)
                .font(.system(size: 9))
                .foregroundColor(.secondary)
            Text(badgeText)
                .font(.system(size: 10, weight: .semibold))
                .foregroundColor(badgeColor)
                .padding(.horizontal, 5)
                .padding(.vertical, 1)
                .background(badgeColor.opacity(0.15))
                .cornerRadius(4)
        }
        .frame(width: 48)
    }

    private var badgeText: String {
        switch support {
        case .Supported: return "YES"
        case .Conditional: return "COND"
        case .Unsupported: return "NO"
        case .Experimental: return "EXP"
        }
    }

    private var badgeColor: Color {
        switch support {
        case .Supported: return .green
        case .Conditional: return .orange
        case .Unsupported: return .secondary
        case .Experimental: return .purple
        }
    }
}
