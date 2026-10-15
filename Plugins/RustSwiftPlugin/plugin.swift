import PackagePlugin
import Foundation

@main
struct RustSwiftPlugin: BuildToolPlugin {
    func createBuildCommands(context: PluginContext, target: Target) async throws -> [Command] {
        guard let sourceTarget = target as? SourceModuleTarget else {
            return []
        }

        let targetDir = sourceTarget.directory
        let fileManager = FileManager.default
        let targetPathString = targetDir.string

        guard let enumerator = fileManager.enumerator(atPath: targetPathString) else {
            return []
        }

        var rustFiles: [Path] = []
        for case let file as String in enumerator {
            if file.hasSuffix(".rs") {
                rustFiles.append(targetDir.appending(subpath: file))
            }
        }

        if rustFiles.isEmpty {
            return []
        }

        let rustSwiftBin = resolveExecutablePath()
        var commands: [Command] = []

        for rustFile in rustFiles {
            let fileName = rustFile.lastComponent
            let baseName = (fileName as NSString).deletingPathExtension
            let outputSwift = context.pluginWorkDirectory.appending(subpath: "Sources/\(target.name)/\(baseName).swift")
            let outputCompat = context.pluginWorkDirectory.appending(subpath: "Sources/\(target.name)/RustCompat.swift")

            commands.append(.buildCommand(
                displayName: "Compiling \(fileName) to Swift via rust-swift",
                executable: Path(rustSwiftBin),
                arguments: [
                    "compile",
                    rustFile.string,
                    "-o", context.pluginWorkDirectory.string,
                    "--package-name", target.name
                ],
                inputFiles: [rustFile],
                outputFiles: [outputSwift, outputCompat]
            ))
        }

        return commands
    }

    private func resolveExecutablePath() -> String {
        if let envBin = ProcessInfo.processInfo.environment["RUST_SWIFT_BIN"],
           FileManager.default.fileExists(atPath: envBin) {
            return URL(fileURLWithPath: envBin).path
        }

        let home = ProcessInfo.processInfo.environment["HOME"] ?? ""
        let searchPaths = [
            "/usr/local/bin/rust-swift",
            "/opt/homebrew/bin/rust-swift",
            "\(home)/.cargo/bin/rust-swift",
            "./target/release/rust-swift",
            "./target/debug/rust-swift",
            "../target/release/rust-swift",
            "../target/debug/rust-swift",
            "../../target/release/rust-swift",
            "../../target/debug/rust-swift"
        ]

        for path in searchPaths {
            if FileManager.default.fileExists(atPath: path) {
                return URL(fileURLWithPath: path).path
            }
        }

        // Search for rust-swift on PATH using /usr/bin/which
        let whichProcess = Process()
        whichProcess.executableURL = URL(fileURLWithPath: "/usr/bin/which")
        whichProcess.arguments = ["rust-swift"]
        let pipe = Pipe()
        whichProcess.standardOutput = pipe
        if (try? whichProcess.run()) != nil {
            whichProcess.waitUntilExit()
            if whichProcess.terminationStatus == 0 {
                let data = pipe.fileHandleForReading.readDataToEndOfFile()
                if let path = String(data: data, encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines),
                   !path.isEmpty, FileManager.default.fileExists(atPath: path) {
                    return URL(fileURLWithPath: path).path
                }
            }
        }

        return URL(fileURLWithPath: "./target/debug/rust-swift").path
    }
}
