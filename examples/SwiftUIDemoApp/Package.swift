// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "SwiftUIDemoApp",
    platforms: [.macOS(.v14)],
    dependencies: [
        .package(path: "../..")
    ],
    targets: [
        .executableTarget(
            name: "SwiftUIDemoApp",
            dependencies: [],
            plugins: [
                .plugin(name: "RustSwiftPlugin", package: "rust-swift")
            ]
        )
    ]
)
