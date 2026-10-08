// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "PluginDemoApp",
    platforms: [.macOS(.v13)],
    dependencies: [
        .package(path: "../..")
    ],
    targets: [
        .executableTarget(
            name: "PluginDemoApp",
            dependencies: [],
            plugins: [
                .plugin(name: "RustSwiftPlugin", package: "rust-swift")
            ]
        )
    ]
)
