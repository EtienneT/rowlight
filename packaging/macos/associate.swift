// Compiled into the bundle; no Swift installation is needed by the user.
import AppKit
import UniformTypeIdentifiers

@main
struct Associate {
    @MainActor
    static func main() async {
        let bundle = URL(fileURLWithPath: CommandLine.arguments[0])
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        let types = Set([UTType(exportedAs: "com.parquetviewer.parquet")]
            + ["parquet", "parq"].compactMap { UTType(filenameExtension: $0) })
        var failures: [String] = []
        for type in types {
            do { try await NSWorkspace.shared.setDefaultApplication(at: bundle, toOpen: type) }
            catch { failures.append(error.localizedDescription) }
        }
        if !failures.isEmpty {
            FileHandle.standardError.write(Data(failures.joined(separator: "\n").utf8))
            exit(1)
        }
    }
}
