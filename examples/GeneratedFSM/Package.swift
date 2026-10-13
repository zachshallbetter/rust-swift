// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "fsm_example",
    platforms: [
        .macOS(.v13),
    ],
    products: [
        .library(
            name: "fsm_example",
            targets: ["fsm_example"]
        ),
    ],
    targets: [
        .target(
            name: "fsm_example",
            path: "Sources/fsm_example"
        ),
    ]
)
