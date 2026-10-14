// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "PeelCoreKit",
    platforms: [
        .macOS(.v13),
    ],
    products: [
        .library(
            name: "PeelCoreKit",
            targets: ["PeelCoreKit"]
        ),
        .executable(
            name: "peel-inspect",
            targets: ["PeelInspectTool"]
        ),
    ],
    targets: [
        .target(
            name: "PeelCoreKit",
            path: "Sources/PeelCoreKit"
        ),
        .executableTarget(
            name: "PeelInspectTool",
            dependencies: ["PeelCoreKit"],
            path: "Sources/PeelInspectTool"
        ),
        .testTarget(
            name: "PeelCoreKitTests",
            dependencies: ["PeelCoreKit"],
            path: "Tests/PeelCoreKitTests"
        ),
    ]
)
