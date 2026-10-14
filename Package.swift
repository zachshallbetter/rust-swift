// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "rust-swift",
    platforms: [
        .macOS(.v13),
        .iOS(.v17),
        .visionOS(.v1)
    ],
    products: [
        .plugin(
            name: "RustSwiftPlugin",
            targets: ["RustSwiftPlugin"]
        )
    ],
    targets: [
        .plugin(
            name: "RustSwiftPlugin",
            capability: .buildTool()
        )
    ]
)
